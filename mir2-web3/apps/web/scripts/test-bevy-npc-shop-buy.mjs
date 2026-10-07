import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// The repository's existing source-test loader. Imports are type-only: no
// runtime/WASM initialization, product socket, browser or gateway dependency.
// Source24 real pure Pearl dependencies; no Core loader or WASM instance.
function loadPearlBuyDependency(relative, dependencies = {}) {
  const source = readFileSync(new URL(relative, import.meta.url), "utf8");
  const output = ts.transpileModule(source, {compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  const loaded = {exports:{}};
  new Function("require","exports","module",output)(name=>{
    assert.ok(Object.hasOwn(dependencies,name),"unexpected Pearl dependency "+name);
    return dependencies[name];
  },loaded.exports,loaded);
  return loaded.exports;
}
const actualCrystalItem=loadPearlBuyDependency("../lib/crystal-item-source.ts");
const actualPearlSource=loadPearlBuyDependency("../lib/npc-pearl-buy-source.ts",{"./crystal-item-source":actualCrystalItem});
const actualPearlBuy=loadPearlBuyDependency("../lib/npc-pearl-buy.ts",{"./npc-pearl-buy-source":actualPearlSource});
const actualDurableNpc=loadPearlBuyDependency("../lib/npc-purchase-client.ts");
test("Source30 durable receiver and retained facade pass strict no-emit type checking", () => {
  const files = ["../lib/npc-purchase-client.ts", "../lib/npc-purchase-receipt.ts"]
    .map(relative => fileURLToPath(new URL(relative, import.meta.url)));
  const program = ts.createProgram(files, {
    noEmit: true, incremental: false, strict: true,
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    lib: ["lib.es2022.d.ts", "lib.dom.d.ts"], types: [],
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  assert.equal(diagnostics.length, 0, ts.formatDiagnostics(diagnostics, {
    getCanonicalFileName: file => file, getCurrentDirectory: () => process.cwd(),
    getNewLine: () => "\n",
  }));
});
const compiled = ts.transpileModule(readFileSync(new URL("../lib/bevy-npc-shop-buy.ts", import.meta.url), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const module = { exports: {} };
new Function("require", "exports", "module", compiled.outputText)(
  name => { if(name === "./npc-pearl-buy") return actualPearlBuy; throw new Error("Unexpected runtime dependency " + name); }, module.exports, module,
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
  operations: "../lib/social-window-operations.ts",
  repair: "../lib/npc-repair-service.ts",
  belt: "../lib/bag-belt-move-dispatcher.ts",
  incoming:"../lib/social-incoming-replies.ts",creature:"../lib/creature-player-ui.ts",cash:"../lib/cash-game-shop-ui.ts",
};
const pagePureRequires = {
  identity: {}, equipment: { "./world-model/item-identity": "identity" },
  parcel: { "./equipment-gateway-adapter": "equipment" },
  storage: { "./equipment-gateway-adapter": "equipment",
    "./world-model/item-identity": "identity", "./mail-parcel-gateway-adapter": "parcel" },
  bag: { "./world-model/item-identity": "identity" },
  operations: {}, repair: {"./equipment-gateway-adapter":"equipment"},
  belt: {"./mail-parcel-gateway-adapter":"parcel"},
  incoming:{},creature:{"./social-incoming-replies":"incoming"},
  cash:{"./social-incoming-replies":"incoming","./creature-player-ui":"creature"},
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
  "itemCommandRequiresOwner", "socialItemMutationAllowed", "parityItemMutationAllowed",
  "npcPearlBuyCurrent", "readNpcPearlBuyCurrent", "readNpcBuyCurrent", "npcPearlSendCurrent",
  "currentSpellsOwner", "currentSocialReplyOwner", "currentSocialReceiveOwner", "sameSocialPhysicalOwner", "observeNpcPearlWallet",
  "npcPurchaseFullEconomyCurrent", "npcPurchaseDisplayFingerprint", "npcPurchaseSnapshotEconomics", "receiveNpcPurchaseGatewayFrame", "applyNpcPurchaseOrdinarySnapshot", "npcPurchaseSessionKey",
  "npcPurchaseShopMatches", "applyNpcPurchaseShopSnapshot", "applyNpcShopCatalogPacket", "dispatchDurableNpcPurchase", "currentNpcShopTab",
  "observeNpcPurchaseSocket", "retireNpcPurchaseClient",
  "closeNpcRepairService", "cancelWorldFishingGesture", "retireWorldFishingGesture", "retainNpcPurchaseSelectors"];
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

// Extract only actual pure auth classifiers; do not initialize login runtime.
const npcAuthAst=ts.createSourceFile("npc-auth-selectors.ts",readFileSync(new URL("../lib/client-login-runtime.ts",import.meta.url),"utf8"),ts.ScriptTarget.Latest,true);
const npcAuthDeclarations=npcAuthAst.statements.filter(node=>ts.isFunctionDeclaration(node)&&["preauthCommandKind","isSensitiveGatewayCommand"].includes(node.name?.text));
assert.equal(npcAuthDeclarations.length,2);
const npcAuthJavaScript=ts.transpileModule(npcAuthDeclarations.map(node=>node.getText(npcAuthAst).replace(/^export /,"")).join("\n"),{compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
const npcAuthSelectors=new Function(npcAuthJavaScript+"\nreturn {preauthCommandKind,isSensitiveGatewayCommand};")();
const npcPureBag = loadPagePure("bag");
const npcPureIdentity = loadPagePure("identity");
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
    npcPurchaseUnavailableRef:{current:false},npcPurchaseApplyingEconomyRef:{current:false},npcPurchaseOptInSocketsRef:{current:new WeakSet()},
    npcPurchaseEconomicSourceRef:{current:null},npcPurchaseDisplaySourceRef:{current:null},
    npcPurchaseCatalogSourceRef:{current:new WeakMap()},npcPurchaseClientRef:{current:null},
    NpcPurchaseClient:actualDurableNpc.NpcPurchaseClient,worldSnapshotVersionRef:{current:0},
    captureQuestMapGatewayEvent:()=>{},captureSpellsGatewayEvent:()=>{},mergeMailList:value=>value,
    mailRawRef:{current:null},socialFriendsRef:{current:null},charactersRef:{current:[]},setCharacters:()=>{},
    completeNpcPurchaseSnapshot:actualDurableNpc.completeNpcPurchaseSnapshot,currentHeroModel:()=>null,
    isNpcPurchaseOwnerFrame:actualDurableNpc.isNpcPurchaseOwnerFrame,isNpcPurchaseWorldFrame:actualDurableNpc.isNpcPurchaseWorldFrame,
    parseNpcPurchaseWorldFrame:actualDurableNpc.parseNpcPurchaseWorldFrame,
    parseGatewayMailDates:expiryPackets.parseGatewayMailDates,
    handleGatewayEvent:(event,generation,socket,raw)=>{
      assert.equal(event.type,"worldSnapshot");scope.ordinaryReceiver(event.payload,raw,generation,socket);
    },
    ordinaryApplications:[],
    applyNpcGoldBuyGatewaySnapshot:(snapshot,generation,full,qualified=false)=>{
      if(!qualified)assert.equal(full,true);assert.equal(generation,scope.equipmentConnectionGenerationRef.current);
      scope.ordinaryApplications.push({full,qualified,snapshot});
      scope.beforeOrdinaryApply?.(snapshot,full,qualified);
      const next={...scope.worldRef.current,...snapshot,playerObjectId:String(snapshot.playerObjectId),connected:true,
        entities:snapshot.entities.map(row=>({...row,objectId:String(row.objectId),classKey:row.class,genderKey:row.gender})),
        inventoryItems:snapshot.inventoryItems.map(row=>({...row,...npcPureIdentity.projectInventoryItemIdentity(row.uniqueId,row.container,row.slot)}))};
      scope.worldRef.current=next;
      const physical={socket:scope.socketRef.current,
        connectionGeneration:generation,sessionGeneration:scope.equipmentSessionGenerationRef.current,playerObjectId:snapshot.playerObjectId,
        sceneRevision:scope.socialSceneRevisionRef.current,mapFileName:snapshot.mapFileName};
      if(snapshot.stage5Systems?.intelligentCreaturePearls!==undefined
        && scope.npcPearlShopSourceRef.current.currentWallet(physical)?.amount!==snapshot.stage5Systems.intelligentCreaturePearls)
        scope.npcPearlShopSourceRef.current.observeWallet(physical,snapshot.stage5Systems.intelligentCreaturePearls);
      const model=npcPureBag.projectBevyBagModel(next,{npcGoldTrade:true});assert(model.ok);
      const owner={connectionGeneration:generation,sessionGeneration:scope.equipmentSessionGenerationRef.current,playerObjectId:snapshot.playerObjectId};
      assert(scope.npcGoldBuyInventoryRef.current.finish(scope.npcGoldBuyInventoryRef.current.begin(owner),owner,true,model.model));
    },
    updateWorld:updater=>{scope.worldRef.current=typeof updater==="function"?updater(scope.worldRef.current):updater;},
    npcPurchaseShopSource:actualDurableNpc.npcPurchaseShopSource,sameNpcPurchaseData:actualDurableNpc.sameNpcPurchaseData,
    npcServiceNameRef:{current:"Shop"},setShowInventory:()=>{},setShowCharacter:()=>{},
    stringOrFallback:(value,fallback)=>typeof value==="string"?value:fallback,numberOrZero:value=>typeof value==="number"?value:0,
    npcShopTabBindingRef:{current:null},isBevyNpcShopUiVariant:()=>false,
    accountIdRef:{current:"actor-account"},socialCharacterIndexRef:{current:{current:1}},
    npcPurchaseSelectorsRef:{current:new WeakMap()},npcPurchaseDecimal:actualDurableNpc.npcPurchaseDecimal,
    heroOperationsRef: {current:{pending:null}}, mailCollectBarrierRef: {current:null},
    ...npcAuthSelectors, npcPearlJson:actualPearlSource.npcPearlJson,parseNpcPearlBuyPlan:actualPearlBuy.parseNpcPearlBuyPlan,
    npcPearlShopSourceRef:{current:new actualPearlSource.NpcPearlShopSource()},
    npcPearlBuyProofsRef:{current:new WeakMap()},npcPearlSendEpochRef:{current:0},
    socialSceneRevisionRef:{current:0}, document:{visibilityState:"visible",hasFocus:()=>true},
    bagBeltMovesRef:{current:new (loadPagePure("belt").BagBeltMoveDispatcher)()},
    bagBeltInventoryReadyRef:{current:null}, equipmentSnapshotRef:{current:null},
    storageRentalRef:{current:{pending:null}},
    cashPurchasesRef:{current:new (loadPagePure("cash").CashGameShopPurchases)()},
    creatureOperationsRef:{current:new (loadPagePure("creature").CreaturePlayerOperations)()},
    npcRepairAuthorityRef:{current:new (loadPagePure("repair").NpcRepairService)()},
    npcRepairDialogBindingRef:{current:null}, setNpcRepairService:()=>{},
    worldFishingGestureRegistryRef:{current:new WeakMap()},
    worldFishingActiveGestureRef:{current:null},worldFishingQueuedRef:{current:null},
    NpcGoldBuyDispatcher, projectBevyBagModel: npcPureBag.projectBevyBagModel,
    npcGoldBuyInventoryRef: { current: new module.exports.NpcGoldBuyInventoryReadiness() },
    mailMutationAllowed: npcPureParcel.mailMutationAllowed,
    isMailItemMutation: npcPureParcel.isMailItemMutation,
    storageMutationAllowed: npcPureStorage.storageMutationAllowed,
    socialItemOperationsRef: { current: new (loadPagePure("operations").SocialWindowOperations)() },
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
  scope.ordinaryReceiver=api.applyNpcPurchaseOrdinarySnapshot;
  // Use the actual Page constructor expression, including its stable runtime
  // getter and read closure. Only trace wrappers surround real methods.
  const dispatcher = api.createActualNpcDispatcher();
  for (const name of ["prepare", "allows", "claim", "preparePearl", "allowsPearl", "claimPearl"]) {
    const actual = dispatcher[name].bind(dispatcher);
    dispatcher[name] = (...args) => {
      trace.push(name); const result = actual(...args);
      if ((name === "prepare" || name === "preparePearl") && result) proofs.push(result);
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
const expiryPackets = loadExpiryPipelineModule("../lib/extended-server-packets.ts", {"./npc-purchase-client":actualDurableNpc});
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
  const oldPackets = loadExpiryPipelineModule("../lib/extended-server-packets.ts", {"./npc-purchase-client":actualDurableNpc}, noSourceJson);
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

// Source24: reuse the existing actual Page harness and scripted Core protocol.
// These host DTOs are fixed ABI replies, never an implementation of the planner.
const pearlHostPlan={version:1,ok:true,maxQuantity:99,quote:34,admittedCount:2,denial:0};
const pearlHostWire={type:"buyItem",itemIndex:0,count:2,panelType:0};
const pearlHostTicket={transport:"1",body:JSON.stringify(pearlHostWire)};
function installPearlPage(f) {
  const rawGood=uid=>{const index=5,info={item_index:index,name:"Same visible name",item_type:1,grade:1,required_type:0,required_class:31,required_gender:3,item_set:0,shape:0,weight:4,light:0,required_amount:5,image:0,durability:4000,stack_size:20,price:900,start_item:false,effect:0,need_identify:false,show_group_pickup:false,class_based:false,level_based:false,can_mine:false,global_drop_notify:false,bind:0,unique:0,random_stats_id:0,can_fast_run:false,can_awakening:false,slots:0,stats:[{stat:4,value:2},{stat:5,value:4}],tooltip:null},
    userItem={unique_id:uid,item_index:index,current_dura:0,max_dura:0,count:1,soul_bound_id:-1,identified:true,cursed:false,slots:[null],gem_count:0,added_stats:[{stat:5,value:3}],awake_type:0,awake_values:[],refined_value:0,refine_added:0,refine_success_chance:0,wedding_ring:-1,expire_info:null,rental_information:null,is_shop_item:true,sealed_info:null,gm_made:false};
    return {...copy(userItem),id:uid,uniqueId:uid,itemIndex:index,name:"Same visible name",price:17,icon:10,grade:1,description:"raw Pearl trade",
      tooltipSource:{info,realInfo:null,userItem,socketInfos:[null],realSocketInfos:[]}};};
  const raw={panelType:0,rate:1.25,list:[rawGood(0),rawGood(42)]};
  const physical=f.currentSocialReplyOwner(false), source=f.scope.npcPearlShopSourceRef.current;
  const catalog=source.observeCatalog(physical,"NPCPearlGoods",raw,()=>true); assert(catalog);
  const wallet=source.observeWallet(physical,200); assert(wallet);
  Object.assign(f.scope.npcShopService,{currency:"pearls",catalogRevision:catalog.revision,
    pearlBalance:wallet.amount,buyItems:catalog.goods.map(good=>({id:good.uniqueId,name:good.name,
      price:good.unitPrice,count:good.count,stock:good.stock,icon:good.icon,description:good.description,
      tooltipSource:good.tooltipSource,requiresPearlBuyPlan:true}))});
  f.scope.world.gold=0;
  const readyOwner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:1};
  f.scope.npcGoldBuyInventoryRef.current.finish(f.scope.npcGoldBuyInventoryRef.current.begin(readyOwner),
    readyOwner,true,npcPureBag.projectBevyBagModel(f.scope.world,{npcGoldTrade:true}).model);
  f.pearlCalls=[];f.pearlAnswer=()=>pearlHostPlan;
  const getter=function(json){f.trace.push("Pearl.planner");f.pearlCalls.push(JSON.parse(json));
    const answer=f.pearlAnswer(JSON.parse(json));return typeof answer==="string"?answer:JSON.stringify(answer);};
  f.scope.questCoreRuntimeRef.current.getMir2NpcPearlBuyPlan=getter;
  f.protocol.ready("9007199254740993","1",pearlHostTicket);
  f.rawPearl=raw;f.physicalPearl=physical;f.pearlGetter=getter;
  return f;
}

test("Pearl actual Page quote uses UID0 and Core scalar custody with zero Gold and no JS price",()=>{
  const f=installPearlPage(npcPageHarness());
  const quote=f.quoteNpcShopItem(0,100);
  assert.equal(quote.currency,"pearls");assert.equal(quote.totalGold,null);assert.equal(quote.totalPearls,34);
  assert.deepEqual(quote.command,pearlHostWire);assert.equal(f.calls.length,0);
  assert.deepEqual(f.pearlCalls,[{allowsBuy:true,selected:true,usePearls:true,uniqueId:0,unitPrice:17,
    stock:-1,quantity:100,walletKnown:true,pearls:200,occupied:1,infoPrice:900,rate:1.25}]);
  assert.equal(f.readNpcGoldBuyCurrent(),null);assert.equal(f.readNpcBuyCurrent().currency,"pearls");
  assert.equal(f.scope.npcShopService.buyItems[0].name,f.scope.npcShopService.buyItems[1].name);
  assert.equal(f.attempts.length,0);
});

test("Pearl actual Page final claim enters the existing Core immediately before exact four-key send",()=>{
  const f=installPearlPage(npcPageHarness());
  f.protocol.set("enter",input=>{f.trace.push("Core.enter");assert.deepEqual(input.ticket,pearlHostTicket);
    return decision("entered",{ticket:pearlHostTicket});});
  f.buyNpcShopItem(0,2,0);
  assert.equal(f.proofs.length,1);assert(Object.isFrozen(f.proofs[0].proof));
  assert.deepEqual(Object.keys(f.proofs[0].proof),[]);assert.deepEqual(f.sent,[pearlHostWire]);
  assert.deepEqual(Object.keys(f.sent[0]).sort(),["count","itemIndex","panelType","type"]);
  assert.deepEqual(f.trace,["preparePearl","Pearl.planner","allowsPearl","Pearl.planner","action","claimPearl","Pearl.planner","Core.enter","socket"]);
  assert.equal(f.protocol.instances.length,1);assert.equal(f.calls.length,0);assert.equal(f.pearlCalls.length,3);
  assert.equal(f.protocol.inputs("receipt")[0].outcome,"flushed");
  assert.equal(f.protocol.inputs("receipt").at(-1).outcome,"definitelyUnsent");
  assert.equal(f.sendRaw(f.proofs[0].wire,{npcPearlBuyProof:f.proofs[0].proof}),false);
  assert.equal(f.attempts.length,1);
});

test("Pearl opaque proof brands reject Gold and legacy routes and cross-currency prepare methods",()=>{
  const f=installPearlPage(npcPageHarness());f.scope.npcBuySelectedRef.current=0;
  const live=f.readNpcPearlBuyCurrent();assert(live);
  assert.equal(f.dispatcher.prepare(live,2),null);assert.equal(f.dispatcher.preview(live,2),null);
  const prepared=f.dispatcher.preparePearl(live,2);assert(prepared);
  assert.equal(f.dispatcher.allows(prepared.proof,prepared.wire),false);
  assert.equal(f.dispatcher.claim(prepared.proof,prepared.wire,JSON.stringify(prepared.wire),f.socket),false);
  assert.equal(f.sendRaw(prepared.wire,{npcBuyProof:prepared.proof}),false);
  assert.equal(f.sendRaw(prepared.wire,{npcLegacyBuyProof:prepared.proof}),false);
  for(const options of [{npcBuyProof:prepared.proof,npcPearlBuyProof:prepared.proof},
    {npcPearlBuyProof:prepared.proof,npcLegacyBuyProof:{}},
    {npcBuyProof:{},npcPearlBuyProof:prepared.proof,npcLegacyBuyProof:{}}])
    assert.equal(f.sendRaw(prepared.wire,options),false);
  assert.equal(f.dispatcher.allowsPearl(prepared.proof,prepared.wire),true);
  const gold=npcPageHarness();gold.scope.npcBuySelectedRef.current=77;
  const goldProof=gold.dispatcher.prepare(gold.readNpcGoldBuyCurrent(),2);assert(goldProof);
  assert.equal(gold.dispatcher.allowsPearl(goldProof.proof,goldProof.wire),false);
  assert.equal(gold.dispatcher.claimPearl(goldProof.proof,goldProof.wire,JSON.stringify(goldProof.wire),gold.socket),false);
  assert.equal(f.attempts.length,0);assert.equal(gold.attempts.length,0);
});

test("Pearl actual Page quantity delegates 99 100 and u16 maximum and rejects absent or aliased UID",()=>{
  for(const quantity of [99,100,65535]){
    const f=installPearlPage(npcPageHarness());f.buyNpcShopItem(0,quantity,0);
    assert.deepEqual(f.sent,[pearlHostWire]);assert.equal(f.pearlCalls[0].quantity,quantity);
    assert(f.pearlCalls.every(input=>input.uniqueId===0&&input.unitPrice===17));
  }
  for(const [id,quantity,panel] of [[0,0,0],[0,65536,0],[5,2,0],[0,2,1],[-1,2,0],[42.5,2,0]]){
    const f=installPearlPage(npcPageHarness());f.buyNpcShopItem(id,quantity,panel);
    assert.equal(f.attempts.length,0);assert.equal(f.proofs.length,0);assert.equal(f.pearlCalls.length,0);
  }
});

test("Pearl actual Page missing or invalid wallet never admits a free host quote",()=>{
  for(const amount of [null,undefined,"0",.5,NaN,Infinity,2147483648]){
    const f=installPearlPage(npcPageHarness());f.scope.npcPearlShopSourceRef.current.observeWallet(f.physicalPearl,amount);
    f.pearlAnswer=()=>({...pearlHostPlan,quote:0});f.buyNpcShopItem(0,2,0);
    assert.equal(f.proofs.length,0);assert.equal(f.attempts.length,0);assert.equal(f.pearlCalls.length,0);
  }
  const known=installPearlPage(npcPageHarness());
  known.scope.npcPearlShopSourceRef.current.observeWallet(known.physicalPearl,-1);
  known.pearlAnswer=()=>({...pearlHostPlan,quote:0});known.buyNpcShopItem(0,2,0);
  assert.deepEqual(known.sent,[pearlHostWire]);assert.equal(known.pearlCalls[0].walletKnown,true);
  assert.equal(known.pearlCalls[0].pearls,-1,"signed wallet delegates normalization to Core/PUI");
});

test("Pearl actual Page wallet catalog and service ABA invalidate exact captured source",()=>{
  for(const mutate of [
    f=>f.scope.npcPearlShopSourceRef.current.observeWallet(f.physicalPearl,200),
    f=>f.scope.npcPearlShopSourceRef.current.observeCatalog(f.physicalPearl,"NPCPearlGoods",f.rawPearl,()=>true),
    f=>{f.scope.npcShopClockRef.current.service++;f.scope.npcShopServiceRef.current={...f.scope.npcShopServiceRef.current,serviceRevision:2};},
    f=>{f.retireNpcShopService();f.scope.npcShopServiceRef.current=f.scope.npcShopService;},
  ]){
    const f=installPearlPage(npcPageHarness());f.onAction(()=>mutate(f));f.buyNpcShopItem(0,2,0);
    assert.equal(f.proofs.length,1);assert.equal(f.attempts.length,0);
    assert.equal(f.protocol.inputs("enter").length,0);
    assert.equal(f.sendRaw(f.proofs[0].wire,{npcPearlBuyProof:f.proofs[0].proof}),false);
  }
});

test("Pearl actual Page socket session and scene changes burn proof and cannot revive on restoration",()=>{
  for(const edge of ["socket","connection","session","scene","map","owner","core","visibility","focus"]){
    const f=installPearlPage(npcPageHarness());const savedCore=f.scope.questCoreRuntimeRef.current;
    f.onAction(()=>{
      if(edge==="socket")f.scope.socketRef.current={readyState:1};
      if(edge==="connection")f.scope.equipmentConnectionGenerationRef.current++;
      if(edge==="session")f.scope.equipmentSessionGenerationRef.current++;
      if(edge==="scene")f.scope.socialSceneRevisionRef.current++;
      if(edge==="map")f.scope.world.mapFileName="D001";
      if(edge==="owner")f.scope.equipmentBagOwnerRef.current.ownerRevision++;
      if(edge==="core")f.scope.questCoreRuntimeRef.current={...savedCore};
      if(edge==="visibility")f.scope.document.visibilityState="hidden";
      if(edge==="focus")f.scope.document.hasFocus=()=>false;
    });f.buyNpcShopItem(0,2,0);
    assert.equal(f.attempts.length,0,edge);assert.equal(f.protocol.inputs("enter").length,0,edge);
    f.scope.socketRef.current=f.socket;f.scope.equipmentConnectionGenerationRef.current=1;
    f.scope.equipmentSessionGenerationRef.current=2;f.scope.socialSceneRevisionRef.current=0;
    f.scope.world.mapFileName="D000";f.scope.equipmentBagOwnerRef.current.ownerRevision=0;
    f.scope.questCoreRuntimeRef.current=savedCore;f.scope.document.visibilityState="visible";
    f.scope.document.hasFocus=()=>true;
    assert.equal(f.sendRaw(f.proofs[0].wire,{npcPearlBuyProof:f.proofs[0].proof}),false,edge);
    assert.equal(f.attempts.length,0,edge);
  }
});

test("Pearl actual Page fresh quote and getter replacement reject after action without legacy downgrade",()=>{
  for(const edge of ["denial","price","getter","getterThrow"]){
    const f=installPearlPage(npcPageHarness());f.onAction(()=>{
      if(edge==="denial")f.pearlAnswer=()=>({...pearlHostPlan,admittedCount:null,denial:7});
      if(edge==="price")f.pearlAnswer=()=>({...pearlHostPlan,quote:35});
      if(edge==="getter")f.scope.questCoreRuntimeRef.current.getMir2NpcPearlBuyPlan=json=>f.pearlGetter(json);
      if(edge==="getterThrow")Object.defineProperty(f.scope.questCoreRuntimeRef.current,"getMir2NpcPearlBuyPlan",{get(){throw Error("getter retired");}});
    });f.buyNpcShopItem(0,2,0);
    assert.equal(f.attempts.length,0);assert.equal(f.calls.length,0);assert.equal(f.legacyProofs.length,0);
    assert.equal(f.protocol.inputs("enter").length,0);
  }
});

test("Pearl actual Page action and final Core entry reentrancy cannot publish a second flight",()=>{
  for(const edge of ["action","enter"]){
    const f=installPearlPage(npcPageHarness());let once=false;
    const reenter=()=>{if(once)return;once=true;f.buyNpcShopItem(0,2,0);};
    if(edge==="action")f.onAction(reenter);
    else f.protocol.set("enter",()=>{reenter();return decision("entered",{ticket:pearlHostTicket});});
    f.buyNpcShopItem(0,2,0);assert.equal(once,true);assert.equal(f.proofs.length,1);
    assert.deepEqual(f.sent,[pearlHostWire]);assert.equal(f.protocol.inputs("bind").length,1);
    assert.equal(f.protocol.inputs("enter").length,1);assert.equal(f.protocol.instances.length,1);
    assert.equal(f.trace.filter(entry=>entry==="action").length,1);
  }
});

test("Pearl entered unknown and flushed Core holds survive ordinary snapshots observe and producer replacement",()=>{
  for(const phase of ["entered","unknown","flushed"]){
    const f=installPearlPage(npcPageHarness());if(phase==="unknown")f.throwSocket();
    f.buyNpcShopItem(0,2,0);assert.equal(f.attempts.length,1);
    // Script a lawful already-held Core state. No host fixture simulates transitions.
    f.protocol.hold(phase,"9007199254740993","1",pearlHostTicket);
    for(const op of ["observe","availability"])f.protocol.set(op,decision(phase,{ticket:pearlHostTicket}));
    f.scope.npcPearlShopSourceRef.current.observeWallet(f.physicalPearl,999);
    f.scope.npcPearlShopSourceRef.current.observeCatalog(f.physicalPearl,"NPCPearlGoods",f.rawPearl,()=>true);
    f.dispatcher.observe();f.buyNpcShopItem(0,2,0);
    assert.equal(f.attempts.length,1);assert.equal(f.dispatcher.status().flight.phase,phase);
    assert.equal(f.quoteNpcShopItem(0,2).blockReason,"localPending");
    const holder=f.slot;f.dispatcher.dispose();f.dispatcher.activate();f.dispatcher.observe();
    assert.strictEqual(f.slot,holder);assert.equal(f.protocol.instances.length,1);
    assert.equal(f.dispatcher.status().flight.phase,phase);assert.equal(f.attempts.length,1);
    assert(f.protocol.inputs("receipt").every(input=>input.ticket.body===JSON.stringify(pearlHostWire)));
  }
});

test("Pearl actual Page own parcel hero and storage reservations reject before Core entry",()=>{
  for(const edge of ["hero","mailCollect","storage","ui"]){
    const f=installPearlPage(npcPageHarness());f.onAction(()=>{
      if(edge==="hero")f.scope.heroOperationsRef.current.pending={state:"entered",proof:{}};
      if(edge==="mailCollect")f.scope.mailCollectBarrierRef.current={};
      if(edge==="storage")f.scope.pendingStorageRequestsRef.current.set(1,{enteredSocket:true});
      if(edge==="ui")f.scope.npcShopUiIngressRef.current={blocksInput:()=>true};
    });f.buyNpcShopItem(0,2,0);
    assert.equal(f.attempts.length,0,edge);assert.equal(f.protocol.inputs("enter").length,0,edge);
  }
});

test("Pearl actual Page retires catalog while retaining personal wallet on service close",()=>{
  const f=installPearlPage(npcPageHarness()), source=f.scope.npcPearlShopSourceRef.current;
  const wallet=source.currentWallet(f.physicalPearl);f.retireNpcShopService();
  assert.equal(source.currentCatalog(f.physicalPearl),null);assert.strictEqual(source.currentWallet(f.physicalPearl),wallet);
  assert.equal(f.scope.npcShopServiceRef.current,null);assert.equal(f.readNpcBuyCurrent(),null);
  assert.equal(f.quoteNpcShopItem(0,2),null);f.buyNpcShopItem(0,2,0);assert.equal(f.attempts.length,0);
});

// Extract the authentic receive fence and its wallet step, plus both complete
// creature-list clauses. The rest of gateway/game projection is outside this fixture.
let pearlReceiveFence,pearlWalletReceiveStep;
const pearlCreatureCases=new Map();
function visitPearlReceive(node){
  if(ts.isFunctionDeclaration(node)&&node.name?.text==="handleGatewayEvent"){
    assert.equal(pearlReceiveFence,undefined);pearlReceiveFence=node.body.statements[0].getText(actualPageAst);
    const steps=node.body.statements.filter(statement=>ts.isIfStatement(statement)
      &&statement.expression.getText(actualPageAst)==='event.type === "packet" && event.packet === "UpdateIntelligentCreatureList"');
    assert.equal(steps.length,1);pearlWalletReceiveStep=steps[0].getText(actualPageAst);
  }
  if(ts.isCaseClause(node)&&ts.isStringLiteral(node.expression)&&["NewIntelligentCreature","UpdateIntelligentCreatureList"].includes(node.expression.text)){
    assert.equal(pearlCreatureCases.has(node.expression.text),false);pearlCreatureCases.set(node.expression.text,node.getText(actualPageAst));
  }
  ts.forEachChild(node,visitPearlReceive);
}
visitPearlReceive(actualPageAst);assert.equal(pearlCreatureCases.size,2);
const pearlReceiveJs=ts.transpileModule('return function(event,connectionGeneration,source){'+pearlReceiveFence+'\n'
  +pearlWalletReceiveStep+'\nconst payload=event.payload??{};switch(event.packet){'
  +["NewIntelligentCreature","UpdateIntelligentCreatureList"].map(name=>pearlCreatureCases.get(name)).join("\n")+'}};',
  {compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;

test("Pearl actual packet wallet fence ignores old socket generation and NewCreature without amount",()=>{
  const f=installPearlPage(npcPageHarness()),source=f.scope.npcPearlShopSourceRef.current;
  f.scope.world.stage5Systems={intelligentCreatures:[]};
  const receive=new Function("equipmentConnectionGenerationRef","socketRef","observeNpcPearlWallet","updateWorld",pearlReceiveJs)(
    f.scope.equipmentConnectionGenerationRef,f.scope.socketRef,f.observeNpcPearlWallet,
    fn=>{f.scope.worldRef.current=fn(f.scope.worldRef.current);});
  const old=source.currentWallet(f.physicalPearl);
  receive({type:"packet",packet:"UpdateIntelligentCreatureList",payload:{pearlCount:999}},0,f.socket);
  receive({type:"packet",packet:"UpdateIntelligentCreatureList",payload:{pearlCount:999}},1,{});
  assert.strictEqual(source.currentWallet(f.physicalPearl),old);
  receive({type:"packet",packet:"NewIntelligentCreature",payload:{creatureList:[{name:"new creature"}]}},1,f.socket);
  assert.strictEqual(source.currentWallet(f.physicalPearl),old);
  assert.deepEqual(f.scope.worldRef.current.stage5Systems.intelligentCreatures,[{name:"new creature"}]);
  receive({type:"packet",packet:"UpdateIntelligentCreatureList",payload:{pearlCount:-1,creatureList:[]}},1,f.socket);
  assert.equal(source.currentWallet(f.physicalPearl).amount,-1);
  assert(source.currentWallet(f.physicalPearl).revision>old.revision);
  receive({type:"packet",packet:"UpdateIntelligentCreatureList",payload:{creatureList:[]}},1,f.socket);
  assert.equal(source.currentWallet(f.physicalPearl),null,"missing amount retires wallet, never substitutes free zero");
});

test("Pearl full snapshot wallet observation remains inside completed owner inventory projection",()=>{
  const found=[];
  function visit(node){if(ts.isFunctionDeclaration(node)&&node.name?.text==="applyNpcGoldBuyGatewaySnapshot")found.push(node);ts.forEachChild(node,visit);}
  visit(actualPageAst);assert.equal(found.length,1);
  const body=found[0].body.statements,tries=body.filter(node=>ts.isTryStatement(node));assert.equal(tries.length,1);
  const complete=tries[0].finallyBlock.statements.find(node=>ts.isIfStatement(node)&&node.expression.getText(actualPageAst)==="complete");
  assert(complete);const calls=complete.thenStatement.statements.filter(node=>node.getText(actualPageAst).includes("observeNpcPearlWallet("));
  assert.equal(calls.length,1);
  assert.equal(calls[0].getText(actualPageAst),'observeNpcPearlWallet((snapshot.stage5Systems as Record<string,unknown> | null)?.intelligentCreaturePearls);');
  const outside=tries[0].finallyBlock.statements.filter(node=>node!==complete).map(node=>node.getText(actualPageAst)).join("\n");
  assert.doesNotMatch(outside,/observeNpcPearlWallet\(/);
  const raw=ts.transpileModule('return function(complete,snapshot){if(complete){'+calls[0].getText(actualPageAst)+'}};',
    {compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
  const f=installPearlPage(npcPageHarness()),source=f.scope.npcPearlShopSourceRef.current;
  const observe=new Function("observeNpcPearlWallet",raw)(f.observeNpcPearlWallet),old=source.currentWallet(f.physicalPearl);
  observe(false,{stage5Systems:{intelligentCreaturePearls:999}});assert.strictEqual(source.currentWallet(f.physicalPearl),old);
  observe(true,{stage5Systems:{intelligentCreaturePearls:0}});assert.equal(source.currentWallet(f.physicalPearl).amount,0);
  observe(true,{stage5Systems:{}});assert.equal(source.currentWallet(f.physicalPearl),null);
});


test("Pearl actual Page foreign proof options and nested send attempts retire the outer Pearl entry",()=>{
  for(const key of ["npcBuyProof","npcLegacyBuyProof","npcUi","npcRepairProof","combatProof","mailProof","cashProof","creatureProof"]){
    const f=installPearlPage(npcPageHarness());let checked=false;
    f.onAction(()=>{
      if(checked)return;checked=true;
      const prepared=f.proofs[0];assert(prepared);
      assert.equal(f.sendRaw(prepared.wire,{npcPearlBuyProof:prepared.proof,[key]:{}}),false,key);
      assert.equal(f.protocol.inputs("enter").length,0,key);assert.equal(f.attempts.length,0,key);
    });
    f.buyNpcShopItem(0,2,0);assert.equal(checked,true);assert.deepEqual(f.sent,[],key);
    assert.equal(f.protocol.inputs("enter").length,0,key);
    assert(f.scope.npcPearlSendEpochRef.current>=2,key);
    f.onAction(null);f.protocol.ready("9007199254740994","1",pearlHostTicket);
    f.buyNpcShopItem(0,2,0);assert.deepEqual(f.sent,[pearlHostWire],key);
    assert.equal(f.protocol.inputs("enter").length,1,key);
  }
});

const pearlGoodsCases=[];
(function visit(node){
  if(ts.isFunctionDeclaration(node)&&node.name?.text==="applyNpcShopCatalogPacket")return;
  if(ts.isCaseClause(node)&&ts.isStringLiteral(node.expression)&&node.expression.text==="NPCPearlGoods")pearlGoodsCases.push(node.getText(actualPageAst));
  ts.forEachChild(node,visit);
})(actualPageAst);
assert.equal(pearlGoodsCases.length,1);
// The actual gateway case now delegates to the sole product catalog reducer.
// Include that complete declaration (and its original source/control fences),
// rather than accidentally counting its internal switch as a second ingress.
const pearlGoodsReceiveJs=ts.transpileModule(npcPageDeclarations.get("applyNpcShopCatalogPacket")
  +'\nreturn function(event,connectionGeneration,source){'+pearlReceiveFence+'\nconst payload=event.payload??{};switch(event.packet){'+pearlGoodsCases[0]+'}};',
  {compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
function bindActualPearlGoodsReceive(f){
  const scope={...f.scope,...f,npcServiceNameRef:{current:"Pearl trader"},
    updateWorld:fn=>{f.scope.worldRef.current=fn(f.scope.worldRef.current);},
    setShowInventory:value=>{f.inventoryVisible=value;},setShowCharacter:value=>{f.characterVisible=value;}};
  const names=Object.keys(scope);return new Function(...names,pearlGoodsReceiveJs)(...names.map(name=>scope[name]));
}

test("Pearl actual typed Goods case obtains a Core denial quote for every complete raw row before exposing UID0",()=>{
  const f=installPearlPage(npcPageHarness());f.pearlCalls.length=0;
  f.pearlAnswer=input=>({...pearlHostPlan,quote:input.unitPrice,admittedCount:null,denial:4});
  const receive=bindActualPearlGoodsReceive(f);
  receive({type:"packet",packet:"NPCPearlGoods",payload:f.rawPearl},1,f.socket);
  const service=f.scope.npcShopServiceRef.current;
  assert.equal(service.currency,"pearls");assert.equal(service.panelType,0);assert.equal(service.pearlBalance,200);
  assert.deepEqual(service.buyItems.map(row=>row.id),[0,42]);assert(service.buyItems.every(row=>row.requiresPearlBuyPlan&&!row.requiresGoldBuyPlan));
  assert.equal(f.pearlCalls.length,2);assert(f.pearlCalls.every(input=>input.walletKnown===false&&input.quantity===1&&input.infoPrice===900&&input.rate===1.25));
  assert.equal(f.calls.length,0);assert.equal(f.attempts.length,0);assert.equal(f.inventoryVisible,false);assert.equal(f.characterVisible,false);
  const raw=f.scope.npcPearlShopSourceRef.current.currentCatalog(f.currentSocialReceiveOwner()).rawPayload;
  assert.deepEqual(raw,f.rawPearl);
});

test("Pearl actual Goods price Oracle missing malformed mismatch and getter custody failures retire all rows",()=>{
  for(const edge of ["missing","badJSON","wrongDenial","priceMismatch","getter","scene"]){
    const f=installPearlPage(npcPageHarness());const receive=bindActualPearlGoodsReceive(f);
    f.pearlAnswer=input=>({...pearlHostPlan,quote:input.unitPrice,admittedCount:null,denial:4});
    if(edge==="missing")delete f.scope.questCoreRuntimeRef.current.getMir2NpcPearlBuyPlan;
    if(edge==="badJSON")f.pearlAnswer=()=>"{";
    if(edge==="wrongDenial")f.pearlAnswer=input=>({...pearlHostPlan,quote:input.unitPrice,admittedCount:null,denial:7});
    if(edge==="priceMismatch")f.pearlAnswer=()=>({...pearlHostPlan,quote:18,admittedCount:null,denial:4});
    if(edge==="getter"||edge==="scene")f.pearlAnswer=input=>{
      if(edge==="getter")f.scope.questCoreRuntimeRef.current.getMir2NpcPearlBuyPlan=json=>f.pearlGetter(json);
      else f.scope.socialSceneRevisionRef.current++;
      return {...pearlHostPlan,quote:input.unitPrice,admittedCount:null,denial:4};
    };
    receive({type:"packet",packet:"NPCPearlGoods",payload:f.rawPearl},1,f.socket);
    assert.equal(f.scope.npcPearlShopSourceRef.current.currentCatalog(f.currentSocialReceiveOwner()),null,edge);
    assert.deepEqual(f.scope.npcShopServiceRef.current.buyItems,[],edge);
    assert.equal(f.scope.npcShopServiceRef.current.panelType,255,edge);assert.equal(f.attempts.length,0);
  }
});

test("Pearl actual Goods case preserves a whole unfiltered catalog and refuses a bad second raw row",()=>{
  const f=installPearlPage(npcPageHarness());const receive=bindActualPearlGoodsReceive(f);
  f.pearlAnswer=input=>({...pearlHostPlan,quote:input.unitPrice,admittedCount:null,denial:4});
  const raw=copy(f.rawPearl);delete raw.list[1].soul_bound_id;
  receive({type:"packet",packet:"NPCPearlGoods",payload:raw},1,f.socket);
  assert.deepEqual(f.scope.npcShopServiceRef.current.buyItems,[]);
  assert.equal(f.scope.npcPearlShopSourceRef.current.currentCatalog(f.currentSocialReceiveOwner()),null);
  assert.equal(f.attempts.length,0);assert.equal(f.calls.length,0);
});


test("Pearl actual final checkpoint rejects facts changed by the last ledger callback after the complete read",()=>{
  for(const edge of ["pending","hidden","focus","hp","dead","catalog","wallet","scene","storage","epoch"]){
    const f=installPearlPage(npcPageHarness());let checked=false;
    f.onAction(()=>{
      if(checked)return;checked=true;const original=f.scope.equipmentControllerRef.current.status;let reads=0;
      f.scope.equipmentControllerRef.current.status=()=>{
        reads++;if(reads===2){
          if(edge==="hidden")f.scope.document.visibilityState="hidden";
          if(edge==="focus")f.scope.document.hasFocus=()=>false;
          if(edge==="hp")f.scope.world.playerHp=0;
          if(edge==="dead")f.scope.world.entities[0].dead=true;
          if(edge==="catalog")f.scope.npcPearlShopSourceRef.current.retireCatalog();
          if(edge==="wallet")f.scope.npcPearlShopSourceRef.current.observeWallet(f.physicalPearl,200);
          if(edge==="scene")f.scope.socialSceneRevisionRef.current++;
          if(edge==="storage")f.scope.pendingStorageRequestsRef.current.set(1,{enteredSocket:true});
          if(edge==="epoch")assert.equal(f.sendRaw({type:"gameShopBuy",gIndex:1,quantity:1,priceType:1}),false);
        }
        return {ready:true,pending:edge==="pending"&&reads>=2?1:0};
      };
      assert.equal(f.npcPearlSendCurrent(f.proofs[0].proof,f.proofs[0].wire,f.socket),false,edge);
      assert.equal(reads,2,edge);if(edge!=="pending")f.scope.equipmentControllerRef.current.status=original;
    });
    f.buyNpcShopItem(0,2,0);assert.equal(checked,true);assert.equal(f.attempts.length,0,edge);
    assert.equal(f.protocol.inputs("enter").length,0,edge);
  }
});

test("Pearl reentrant rejected other command still spends send epoch and cannot revive the older proof",()=>{
  const f=installPearlPage(npcPageHarness());let nested=false;
  f.onAction(()=>{if(nested)return;nested=true;const service=f.scope.npcShopServiceRef.current;
    const wallet=f.scope.npcPearlShopSourceRef.current.currentWallet(f.physicalPearl);
    assert.equal(f.sendRaw({type:"gameShopBuy",gIndex:1,quantity:1,priceType:1}),false);
    assert.strictEqual(f.scope.npcShopServiceRef.current,service);
    assert.strictEqual(f.scope.npcPearlShopSourceRef.current.currentWallet(f.physicalPearl),wallet);
  });
  f.buyNpcShopItem(0,2,0);assert.equal(nested,true);assert.equal(f.attempts.length,0);assert.equal(f.protocol.inputs("enter").length,0);
  const old=f.proofs[0];assert.equal(f.sendRaw(old.wire,{npcPearlBuyProof:old.proof}),false);
  assert.equal(f.attempts.length,0);assert(f.scope.npcPearlSendEpochRef.current>=3);
  f.onAction(null);f.protocol.ready("9007199254740994","1",pearlHostTicket);
  f.buyNpcShopItem(0,2,0);assert.deepEqual(f.sent,[pearlHostWire]);assert.equal(f.proofs.length,2);
  assert.notStrictEqual(f.proofs[0].proof,f.proofs[1].proof);assert.equal(f.protocol.inputs("enter").length,1);
});


test("Pearl actual Page quote and exact send retain an above16KiB complete structured catalog through original payload mutation",()=>{
  const f=installPearlPage(npcPageHarness());
  const raw={panelType:0,rate:1.25,list:Array.from({length:8},(_,uid)=>{
    const row=copy(f.rawPearl.list[0]);row.id=uid;row.uniqueId=uid;row.unique_id=uid;
    row.tooltipSource.userItem.unique_id=uid;row.description="p".repeat(4096);return row;
  })},before=copy(raw);assert(Buffer.byteLength(JSON.stringify(raw))>16384);
  const catalog=f.scope.npcPearlShopSourceRef.current.observeCatalog(f.physicalPearl,"NPCPearlGoods",raw,()=>true);assert(catalog);
  f.scope.npcShopService.catalogRevision=catalog.revision;
  f.scope.npcShopService.buyItems=catalog.goods.map(row=>({id:row.uniqueId,name:row.name,price:row.unitPrice,
    count:row.count,stock:row.stock,icon:row.icon,tooltipSource:row.tooltipSource,description:row.description,requiresPearlBuyPlan:true}));
  assert.equal(f.quoteNpcShopItem(0,2).totalPearls,34);
  assert.deepEqual(f.readNpcPearlBuyCurrent().catalog.rawPayload,before);
  f.onAction(()=>{raw.list[0].unique_id=999;raw.list[0].price=0;raw.list[0].tooltipSource.userItem.added_stats[0].value=999;});
  f.buyNpcShopItem(0,2,0);assert.deepEqual(f.sent,[pearlHostWire]);assert.equal(f.protocol.inputs("enter").length,1);
  const lease=f.scope.npcPearlBuyProofsRef.current.get(f.proofs[0].proof);assert(lease);
  assert.deepEqual(JSON.parse(lease.currentJson).catalog.rawPayload,before);
  assert.deepEqual(catalog.rawPayload,before);assert(Object.isFrozen(catalog.rawPayload.list[0].tooltipSource.userItem));
  assert.equal(f.calls.length,0);assert.equal(f.pearlCalls.length,4);
});


// Source29: run the actual plain TypeScript receipt facade through this existing
// CPU-only loader. Fixed ABI responses are custody fixtures, not another Core
// ledger, allocator, pricing algorithm or application-witness implementation.
const receiptFacade = loadPearlBuyDependency("../lib/npc-purchase-receipt.ts", {"./npc-purchase-client":actualDurableNpc});
const receiptTokens = ["0".repeat(62) + "a1", "0".repeat(62) + "a2", "0".repeat(62) + "a3"];
const originalReceiptOperation = Object.freeze({ actor: "1".repeat(64), requestScope: "2".repeat(64),
  sequence: "9007199254740993", intent: { request: { itemIndex: "0", count: 1, panelType: 0 },
    currency: "gold", source: "trade", serviceCatalogProof: "3".repeat(64) } });
function receiptFacadeOracle() {
  const inputs = [], instances = [], connections = [...receiptTokens], responses = new Map();
  class Bridge {
    constructor() { instances.push(this); }
    transact(raw) {
      const request = JSON.parse(raw); inputs.push(request);
      if (responses.has(request.op)) {
        const response = responses.get(request.op);
        return typeof response === "function" ? response(request) : response;
      }
      if (request.op === "openConnection") return JSON.stringify({ ok: true, token: connections.shift() });
      if (request.op === "status") return JSON.stringify({ ok: true, pending: originalReceiptOperation });
      return JSON.stringify({ ok: true, matched: true });
    }
  }
  return { inputs, instances, responses,
    module: { npc_purchase_receipt_abi_version: () => 1, NpcPurchaseReceiptBridge: Bridge } };
}
function liveReceiptFacade(oracle = receiptFacadeOracle(), owner = {}) {
  const runtime = receiptFacade.persistentNpcPurchaseReceiptHost(oracle.module, owner, "source29"), source = {}, socket = {};
  assert.equal(runtime.attachProducer(source), true);
  assert.equal(runtime.transportFor(source, socket, socket, true), receiptTokens[0]);
  return { oracle, owner, runtime, source, socket };
}
test("receipt_facade_document_remount_retains_one_host_and_original_operation", () => {
  const { oracle, owner, runtime, source, socket } = liveReceiptFacade();
  const successor = receiptFacadeOracle();
  assert.equal(receiptFacade.persistentNpcPurchaseReceiptHost(successor.module, owner, "source29"), runtime);
  assert.equal(oracle.instances.length, 1); assert.equal(successor.instances.length, 0);
  assert.deepEqual(runtime.transact(source, socket, { op: "status" }).pending, originalReceiptOperation);
  assert.equal(runtime.transact(source, socket, { op: "unknown", binding: {}, operation: originalReceiptOperation }).ok, true);
  const actual = oracle.inputs.at(-1);
  assert.equal(actual.token, receiptTokens[0]); assert.deepEqual(actual.operation, originalReceiptOperation);
  const descriptor = Object.getOwnPropertyDescriptor(owner, Symbol.for("mir2.clientCore.npcPurchaseReceipt.v1"));
  assert.equal(descriptor.configurable, false); assert.equal(descriptor.writable, false);
});
test("receipt_facade_version_mismatch_withdraws_but_never_recreates_retained_host", () => {
  const { oracle, owner, runtime, source, socket } = liveReceiptFacade();
  const successor = receiptFacadeOracle();
  assert.equal(receiptFacade.persistentNpcPurchaseReceiptHost(successor.module, owner, "different-build"), runtime);
  assert.deepEqual(oracle.inputs.at(-1), { op: "withdraw", token: receiptTokens[0], disconnect: true });
  assert.equal(successor.instances.length, 0); assert.equal(oracle.instances.length, 1);
  assert.equal(runtime.transact(source, socket, { op: "status" }).ok, false);
  assert.equal(runtime.attachProducer({}), false);
});
test("receipt_facade_old_cleanup_cannot_withdraw_successor_or_borrow_its_socket", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade(), nextSource = {}, nextSocket = {};
  assert.equal(runtime.attachProducer(nextSource), true);
  assert.equal(runtime.transportFor(nextSource, nextSocket, nextSocket, true), receiptTokens[1]);
  const count = oracle.inputs.length;
  assert.equal(runtime.withdrawProducer(source), false);
  assert.equal(runtime.transact(source, nextSocket, { op: "unknown", operation: originalReceiptOperation }).ok, false);
  assert.equal(runtime.transact(source, nextSocket, { op: "begin" }).ok, false);
  assert.equal(oracle.inputs.length, count);
  assert.equal(runtime.transact(nextSource, nextSocket, { op: "begin" }).ok, true);
  assert.equal(oracle.inputs.at(-1).token, receiptTokens[1]);
  assert.equal(runtime.transact(source, socket, { op: "unknown", operation: originalReceiptOperation }).ok, true);
  assert.equal(oracle.inputs.at(-1).token, receiptTokens[0]);
});
test("receipt_facade_reconnect_tokens_remain_bound_to_original_physical_socket", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade(), nextSocket = {};
  assert.equal(runtime.transportFor(source, socket, socket, true), receiptTokens[0]);
  assert.equal(runtime.transportFor(source, {}, nextSocket, true), null);
  assert.equal(runtime.transportFor(source, nextSocket, nextSocket, false), null);
  assert.equal(runtime.transportFor(source, nextSocket, nextSocket, true), receiptTokens[1]);
  assert.equal(runtime.transportFor(source, socket, socket, true), null);
  const count = oracle.inputs.length;
  assert.equal(runtime.transact(source, socket, { op: "begin" }).ok, false);
  assert.equal(oracle.inputs.length, count);
  assert.equal(runtime.transact(source, socket, { op: "cancelUnsent", operation: originalReceiptOperation }).ok, true);
  assert.equal(oracle.inputs.at(-1).token, receiptTokens[0]);
});
test("receipt_facade_operation_accessor_is_rejected_without_invocation", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade();
  let getterCalls = 0;
  const input = Object.defineProperty({}, "op", { enumerable: true, get() { getterCalls++; return getterCalls === 1 ? "unknown" : "begin"; } });
  const count = oracle.inputs.length;
  assert.throws(() => runtime.transact(source, socket, input), /Accessor/);
  assert.equal(getterCalls, 0); assert.equal(oracle.inputs.length, count);
  assert.equal(runtime.transact(source, socket, { op: "begin" }).ok, true);
});
test("receipt_facade_nested_nondata_and_lossy_values_never_enter_abi", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade();
  let hooks = 0;
  const getter = Object.defineProperty({}, "itemIndex", { enumerable: true, get() { hooks++; return "0"; } });
  const toJSON = { toJSON() { hooks++; return {}; } };
  const symbol = { [Symbol("hidden")]: 1 };
  const unsafe = { sequence: 9007199254740992 };
  const sparse = new Array(2); sparse[1] = "x";
  for (const nested of [getter, toJSON, symbol, unsafe, sparse, Object.create({ inherited: "x" })]) {
    const count = oracle.inputs.length;
    assert.throws(() => runtime.transact(source, socket, { op: "query", nested }));
    assert.equal(oracle.inputs.length, count);
  }
  assert.equal(hooks, 0);
});
test("receipt_facade_nested_proxy_reentry_cannot_enter_after_producer_transition", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade(), nextSource = {};
  let reflected = 0;
  const nested = new Proxy({ value: "0" }, { ownKeys(target) { reflected++; runtime.attachProducer(nextSource); return Reflect.ownKeys(target); } });
  assert.equal(runtime.transact(source, socket, { op: "begin", nested }).ok, false);
  assert.equal(reflected, 1); assert.equal(oracle.inputs.filter(input => input.op === "begin").length, 0);
  assert.deepEqual(oracle.inputs.at(-1), { op: "withdraw", token: receiptTokens[0], disconnect: true });
  const nextSocket = {};
  assert.equal(runtime.transportFor(nextSource, nextSocket, nextSocket, true), receiptTokens[1]);
  assert.equal(runtime.transact(nextSource, nextSocket, { op: "begin" }).ok, true);
});
test("receipt_facade_caller_cannot_supply_core_token_or_open_its_own_connection", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade(), count = oracle.inputs.length;
  for (const input of [{ op: "openConnection" }, { op: "begin", token: receiptTokens[1] }]) {
    assert.throws(() => runtime.transact(source, socket, input), /Invalid economic facade request/);
  }
  assert.equal(oracle.inputs.length, count);
});
test("receipt_facade_bound_bridge_method_survives_old_instance_method_replacement", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade();
  oracle.instances[0].transact = () => { throw Error("replacement must not execute"); };
  assert.equal(runtime.transact(source, socket, { op: "begin" }).ok, true);
  assert.equal(oracle.inputs.at(-1).token, receiptTokens[0]);
});
test("receipt_facade_invalid_output_poison_keeps_original_host_unavailable", () => {
  const { oracle, owner, runtime, source, socket } = liveReceiptFacade();
  oracle.responses.set("begin", '{"ok":false,"error":"refused","extra":true}');
  assert.equal(runtime.transact(source, socket, { op: "begin" }).ok, false);
  assert.deepEqual(oracle.inputs.at(-1), { op: "withdraw", token: receiptTokens[0], disconnect: true });
  const replacement = receiptFacadeOracle();
  assert.equal(receiptFacade.persistentNpcPurchaseReceiptHost(replacement.module, owner, "source29"), runtime);
  assert.equal(replacement.instances.length, 0);
  assert.equal(runtime.transact(source, socket, { op: "status" }).ok, false);
});
test("receipt_facade_unavailable_abi_is_persistent_and_does_not_admit_producer", () => {
  const owner = {}, unavailableModule = { npc_purchase_receipt_abi_version: () => 0 };
  const runtime = receiptFacade.persistentNpcPurchaseReceiptHost(unavailableModule, owner, "source29");
  assert.equal(runtime.attachProducer({}), false);
  const replacement = receiptFacadeOracle();
  assert.equal(receiptFacade.persistentNpcPurchaseReceiptHost(replacement.module, owner, "source29"), runtime);
  assert.equal(replacement.instances.length, 0);
});
test("receipt_facade_valid_wire_operation_retains_canonical_u64_strings_and_zero_selector", () => {
  const { oracle, runtime, source, socket } = liveReceiptFacade();
  const input = Object.freeze({ op: "unknown", operation: originalReceiptOperation });
  assert.equal(runtime.transact(source, socket, input).ok, true);
  const actual = oracle.inputs.at(-1);
  assert.equal(actual.operation.sequence, "9007199254740993");
  assert.equal(actual.operation.intent.request.itemIndex, "0");
  assert.deepEqual(actual.operation, originalReceiptOperation);
});

// Source30: actual pure socket adapter with explicitly scripted ABI DTOs. This
// oracle is not Core, a WASM instance, a gateway or a durable journal.
const s30Actor="11".repeat(32),s30Scope="22".repeat(32),s30Proof="33".repeat(32);
const s30Binding={actor:s30Actor,producerScope:s30Scope,beginId:"1"};
const s30Authority={actor:s30Actor,producerScope:s30Scope,serverRevision:"0"};
const s30Request={itemIndex:"0",count:2,panelType:0};
const s30Intent={request:s30Request,currency:"gold",source:"trade",serviceCatalogProof:s30Proof};
const s30Operation={actor:s30Actor,requestScope:"44".repeat(32),sequence:"9007199254740993",intent:s30Intent};
function s30Same(a,b){try{assert.deepEqual(copy(a),copy(b));return true;}catch{return false;}}
function s30Snapshot(){
  const stage5Systems={group:{},guild:{},social:{},
    relationship:{allowLoverRecall:false,cooldownUntilMs:0,allowMarriage:false,partnerName:"",marriedDateBinaryDatetime:0,mapName:"",marriedDays:0,pendingRequestFrom:null,pendingDivorceFrom:null},
    mentor:{isMentor:false,cooldownUntilMs:0,allowMentor:false,name:"",level:0,online:false,menteeExp:0,pendingRequestFrom:null,pendingRequestLevel:0},mail:[],gameShopIndividualPurchases:{},
    economyProjectionEventIds:[],trade:null,auction:[],refine:{},conquest:{},guildTerritory:{},hero:null,heroLearnedMagics:[],
    profession:{},appearance:{},nameLists:[],intelligentCreatures:[],summonedIntelligentCreatureType:null,
    intelligentCreaturePearls:0,itemRental:{},attackMode:0,petMode:0,pkDecayElapsedTicks:0};
  return {tick:0,mapTitle:"Bichon",mapFileName:"0",inSafeZone:true,lightSetting:0,playerObjectId:7,
    playerHp:100,playerMaxHp:100,playerMp:40,playerMaxMp:40,playerCrystalStats:[],playerPkPoints:0,
    playerExperience:0,playerMaxExperience:100,gold:100,credit:0,cityCurrencies:{},currentWeight:0,playerWeights:null,
    maxWeight:100,freeBagSlots:40,maxBagSlots:40,inventoryCapacity:46,npcGoldTradeCapacity:null,storageSize:80,
    hasExpandedStorage:false,hasStoragePassword:false,requireStoragePassword:false,storagePasswordLastSetBinaryDatetime:0,
    expandedStorageExpiryTimeBinaryDatetime:0,entities:[{objectId:7,name:"Actor",class:"Warrior",gender:"Male",level:1,x:10,y:10,direction:"Down",dead:false}],
    beltItems:[],inventoryItems:[],storageItems:[],equipmentItems:[],heroInventoryItems:[],heroEquipmentItems:[],heroInventoryCapacity:0,
    heroStats:[],heroVitals:null,heroWeights:{bag:0,wear:0,hand:0},questLog:[],knownSkills:[],activeBuffs:[],activeNpcDialog:null,
    stage5Systems,terrainPatches:[],decorObjects:[],groundDrops:[],mapTransfers:[],interactionHints:[],sceneView:null};
}
function s30Harness(){
  const inputs=[],sent=[],applied=[],source={socket:{}},state={current:true,session:"actor-session",old:false,pending:null,
    throwPurchase:false,refuseEnter:false,applyComplete:true,gesture:true,binding:copy(s30Binding),intent:copy(s30Intent),operation:copy(s30Operation),mutate:null};
  let control=0;
  const host={attachProducer:()=>true,withdrawProducer:()=>true,transportFor:()=>"55".repeat(32),transact(_producer,_socket,input){
    inputs.push(copy(input));let result;
    if(input.op==="status")result={ok:true,token:"55".repeat(32),binding:state.binding,pending:state.pending,actors:[],controls:0};
    else if(["begin","quote","query","enter"].includes(input.op)){
      if(input.op==="enter"&&state.refuseEnter)return {ok:false,error:"entry refused"};
      const action=input.op==="begin"?{kind:"begin"}:input.op==="quote"?{kind:"quote",request:input.request}:
        {kind:input.op==="enter"?"purchase":"query",operation:input.operation??state.operation};
      const request={type:"npcPurchaseOwner",protocolVersion:1,requestId:String(++control),action};
      if(input.op==="begin")state.binding={...copy(s30Binding),beginId:request.requestId};
      if(input.op==="enter"&&state.pending)state.pending={...state.pending,phase:"entered"};
      result={ok:true,request,body:JSON.stringify(request)};
    }else if(input.op==="receive"){
      let frame;try{frame=JSON.parse(input.frame);}catch{return {ok:false,error:"strict raw rejected"};}
      result={ok:true,frame,observation:{kind:"ignored"},kind:frame.reply.kind==="producer"?"producer":frame.reply.kind==="quote"?"quote":"result"};
      if(result.kind==="producer")result.binding=state.binding;
      if(result.kind==="quote")result.intent=frame.reply.intent;
    }else if(input.op==="reserve"){
      state.pending={operation:copy(state.operation),minimumRevision:"1",phase:"queued"};result={ok:true,operation:state.operation};
    }else if(input.op==="cancelUnsent"){
      const matched=state.pending?.phase==="queued"&&s30Same(state.pending.operation,input.operation);
      if(matched)state.pending=null;result={ok:true,matched};
    }else if(input.op==="unknown"){
      const matched=!!state.pending&&state.pending.phase!=="queued"&&s30Same(state.pending.operation,input.operation);
      if(matched)state.pending={...state.pending,phase:"unknown"};result={ok:true,matched};
    }
    else if(input.op==="applied")result={ok:true,observation:{kind:"pending"}};
    else result={ok:true,matched:true};
    return state.mutate?.(input,result)??copy(result);
  }};
  const client=new actualDurableNpc.NpcPurchaseClient({host,socket:source.socket,current:()=>state.current,session:()=>state.session,
    oldAttemptBlocked:()=>state.old,send:body=>{sent.push(JSON.parse(body));if(state.throwPurchase&&sent.at(-1).action.kind==="purchase")throw Error("send Unknown");},
    apply:(snapshot,raw)=>{applied.push({snapshot,raw});return state.applyComplete;}});
  const frame=(requestId,reply,snapshot=null,authority=null)=>JSON.stringify({type:"npcPurchaseOwner",protocolVersion:1,requestId,reply,snapshot,authority});
  const begin=()=>{assert(client.open());assert(client.begin());const id=sent.at(-1).requestId;client.receive(frame(id,{kind:"producer",producer:s30Authority},s30Snapshot(),s30Authority));};
  const quote=()=>client.receive(frame(sent.at(-1).requestId,{kind:"quote",intent:state.intent}));
  return {client,host,inputs,sent,applied,state,frame,begin,quote,ops:op=>inputs.filter(input=>input.op===op)};
}
test("Source30 raw parser rejects duplicate escaped duplicate unsafe integer and preserves canonical u64 strings",()=>{
  for(const raw of ['{"id":0,"id":1}','{"id":0,"\\u0069d":1}','{"uniqueId":9007199254740993}','{"tick":1e20}'])assert.throws(()=>actualDurableNpc.parseNpcPurchaseJson(raw));
  const v=actualDurableNpc.parseNpcPurchaseJson('{"uniqueId":0,"itemIndex":"18446744073709551615","sequence":"9007199254740993"}');
  assert.equal(v.uniqueId,0);assert.equal(v.itemIndex,"18446744073709551615");assert(Object.isFrozen(v));
  assert(actualDurableNpc.npcPurchaseDecimal("0"));assert(actualDurableNpc.npcPurchaseDecimal("18446744073709551615"));
  for(const value of [0,"01","18446744073709551616","-1"])assert.equal(actualDurableNpc.npcPurchaseDecimal(value),false);
});
test("Source30 numeric token boundaries reject fractional identity rounding and bounded exponent underflow",()=>{
  for(const token of ["9007199254740991.1","9007199254740990.9","-9007199254740991.1","9.0071992547409911e15",
    "1.00000000000000001","1e-400","-1e-400","1e-9999999999999999999999","1e9999999999999999999999"]){
    assert.throws(()=>actualDurableNpc.parseNpcPurchaseJson('{"uniqueId":'+token+'}'),undefined,token);
  }
  for(const [token,expected] of [["9007199254740991.0",9007199254740991],["9.007199254740991e15",9007199254740991],
    ["1e3",1000],["1.00",1],["0.1",0.1],["-0",0]]){
    const parsed=actualDurableNpc.parseNpcPurchaseJson('{"value":'+token+'}');assert.equal(parsed.value===expected,true,token);
  }
});
test("Source30 public complete economy rejects omitted partial wallets stage5 hero or actor fields",()=>{
  assert(actualDurableNpc.completeNpcPurchaseSnapshot(s30Snapshot()));
  for(const key of ["stage5Systems","gold","playerExperience","equipmentItems","heroInventoryItems","storageItems","playerWeights"]){const v=s30Snapshot();delete v[key];assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false,key);}
  for(const key of ["mentor","relationship","mail","economyProjectionEventIds","intelligentCreaturePearls"]){const v=s30Snapshot();delete v.stage5Systems[key];assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false,key);}
  const v=s30Snapshot();v.inventoryItems=[{uniqueId:"9007199254740993"}];assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false);
});
test("Source30 actual WorldItem hero equipment and nullable u32 player weights match simulation client view",()=>{
  const v=s30Snapshot();v.heroInventoryCapacity=46;
  v.heroEquipmentItems=[{key:"woodenSword",name:"Wooden Sword",description:"Sword",icon:1,uniqueId:0,slot:0,container:"bag1",quantity:1,
    durabilityCurrent:1,durabilityMax:10,sellValue:0,equipSlot:"weapon",grade:"none",addedAttack:0,addedDefence:0}];
  v.playerWeights={bag:0,wear:4294967295,hand:1};assert(actualDurableNpc.completeNpcPurchaseSnapshot(v));
  v.heroEquipmentItems[0].slot="weapon";assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false);
  v.heroEquipmentItems=[];
  for(const weights of [{bag:0,wear:0},{bag:0,wear:0,hand:-1},{bag:0,wear:0,hand:4294967296},[],false]){
    v.playerWeights=weights;assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false);
  }
  v.playerWeights=null;assert(actualDurableNpc.completeNpcPurchaseSnapshot(v));
});
test("Source30 paired Begin applies frozen full source before exact complete witness",()=>{
  const f=s30Harness();f.begin();assert.equal(f.applied.length,1);assert(Object.isFrozen(f.applied[0].snapshot.stage5Systems));
  assert.deepEqual(f.ops("applied"),[{op:"applied",binding:s30Binding,authority:s30Authority,complete:true}]);
  assert.equal(f.ops("query").length,0);assert(f.inputs.findIndex(i=>i.op==="receive")<f.inputs.findIndex(i=>i.op==="applied"));
});
test("Source30 partial application unsafe original integer and omitted bundle never issue complete witness",()=>{
  for(const edge of ["apply","unsafe","missing","wrongProducer"]){const f=s30Harness();assert(f.client.open());assert(f.client.begin());
    const snapshot=s30Snapshot();if(edge==="apply")f.state.applyComplete=false;if(edge==="missing")delete snapshot.storageItems;
    let raw=f.frame("1",{kind:"producer",producer:s30Authority},snapshot,s30Authority);
    if(edge==="unsafe")raw=raw.replace('"tick":0','"tick":9007199254740993');
    if(edge==="wrongProducer")raw=raw.replace('"producer":{"actor":"'+s30Actor,'"producer":{"actor":"'+"66".repeat(32));
    f.client.receive(raw);assert.equal(f.ops("applied").length,0,edge);
  }
});
for(const [currency,source] of [["gold","trade"],["gold","buyBack"],["gold","used"],["pearls","trade"]]){
  test(`Source30 actual ${currency} ${source} explicit gesture Quote Reserve Enter sends once`,()=>{
    const f=s30Harness();f.state.intent={...copy(s30Intent),currency,source};f.state.operation={...copy(s30Operation),intent:f.state.intent};
    f.begin();assert(f.client.purchase(s30Request,currency,()=>f.state.gesture));f.quote();f.quote();
    assert.equal(f.ops("reserve").length,1);assert.equal(f.ops("enter").length,1);
    const sent=f.sent.filter(value=>value.action.kind==="purchase");assert.equal(sent.length,1);assert.deepEqual(sent[0].action.operation,f.state.operation);
    assert.equal(f.sent.some(value=>value.type==="buyItem"),false);
  });
}
test("Source30 quote rejects changed source currency request and stale physical owner before reserve",()=>{
  for(const edge of ["gesture","socket","currency","request","binding"]){const f=s30Harness();f.begin();assert(f.client.purchase(s30Request,"gold",()=>f.state.gesture));
    if(edge==="gesture")f.state.gesture=false;if(edge==="socket")f.state.current=false;
    if(edge==="currency")f.state.intent.currency="pearls";if(edge==="request")f.state.intent.request.count=3;
    if(edge==="binding")f.client.withdraw();f.quote();assert.equal(f.ops("reserve").length,0,edge);assert.equal(f.ops("enter").length,0,edge);
  }
});
test("Source30 Enter send exception marks original Unknown and fresh Begin queries original operation only",()=>{
  const f=s30Harness();f.begin();assert(f.client.purchase(s30Request,"gold",()=>true));f.state.throwPurchase=true;f.quote();
  assert.equal(f.ops("unknown").length,1);assert.deepEqual(f.ops("unknown")[0].operation,s30Operation);
  f.state.pending={operation:s30Operation,minimumRevision:"1",phase:"unknown"};f.client.withdraw();assert(f.client.begin());
  const id=f.sent.at(-1).requestId;f.client.receive(f.frame(id,{kind:"producer",producer:s30Authority},s30Snapshot(),s30Authority));
  assert.equal(f.ops("query").length,1);assert.deepEqual(f.sent.at(-1).action,{kind:"query",operation:s30Operation});
  assert.equal(f.ops("reserve").length,1);assert.equal(f.ops("enter").length,1);assert.equal(f.sent.filter(r=>r.action.kind==="purchase").length,1);
});
test("Source30 post Enter callbacks and malformed decisions retain Unknown original without Purchase or retry",()=>{
  for(const edge of ["gesture","gestureThrows","oldBarrier","epoch","body","extra","throw"]){
    const f=s30Harness();f.begin();assert(f.client.purchase(s30Request,"gold",()=>{
      if(edge==="gestureThrows"&&f.state.pending?.phase==="entered")throw Error("late callback failed");return f.state.gesture;
    }));
    f.state.mutate=(input,result)=>{
      if(input.op!=="enter")return result;
      if(edge==="gesture")f.state.gesture=false;if(edge==="oldBarrier")f.state.old=true;if(edge==="epoch")f.client.withdraw();
      if(edge==="body")result.body='{"type":"buyItem"}';if(edge==="extra")result.extra=true;if(edge==="throw")throw Error("ABI decision lost");return result;
    };
    f.quote();assert.equal(f.ops("enter").length,1,edge);assert.equal(f.ops("unknown").length,1,edge);
    assert.equal(f.ops("cancelUnsent").length,0,edge);assert.equal(f.state.pending.phase,"unknown",edge);
    assert.deepEqual(f.state.pending.operation,s30Operation);assert.equal(f.sent.filter(row=>row.action.kind==="purchase").length,0,edge);
    f.state.mutate=null;f.state.gesture=true;f.state.old=false;f.client.withdraw();assert(f.client.begin());
    f.client.receive(f.frame(f.sent.at(-1).requestId,{kind:"producer",producer:s30Authority},s30Snapshot(),s30Authority));
    assert.equal(f.ops("query").length,1,edge);assert.deepEqual(f.sent.at(-1).action,{kind:"query",operation:s30Operation});
    assert.equal(f.ops("reserve").length,1,edge);assert.equal(f.ops("enter").length,1,edge);
  }
});
test("Source30 malformed Reserve and refused Enter cancel only exact queued definitely unsent custody",()=>{
  for(const edge of ["reserveExtra","reserveOperation","reserveThrow","changedGesture","queuedCallbackThrows","refusedEnter"]){
    const f=s30Harness();f.begin();assert(f.client.purchase(s30Request,"gold",()=>{
      if(edge==="queuedCallbackThrows"&&f.state.pending?.phase==="queued")throw Error("pre-entry callback failed");return f.state.gesture;
    }));
    f.state.refuseEnter=edge==="refusedEnter";
    f.state.mutate=(input,result)=>{
      if(input.op!=="reserve")return result;
      if(edge==="reserveExtra")return {...result,extra:true};if(edge==="reserveOperation")return {...result,operation:{...result.operation,sequence:0}};
      if(edge==="reserveThrow")throw Error("reserve decision lost");if(edge==="changedGesture")f.state.gesture=false;return result;
    };
    f.quote();assert.equal(f.ops("cancelUnsent").length,1,edge);assert.deepEqual(f.ops("cancelUnsent")[0].operation,s30Operation);
    assert.equal(f.state.pending,null,edge);assert.equal(f.sent.filter(row=>row.action.kind==="purchase").length,0,edge);
    assert.equal(f.ops("enter").length,edge==="refusedEnter"?1:0,edge);
  }
  const f=s30Harness();f.begin();assert(f.client.purchase(s30Request,"gold",()=>true));
  f.state.mutate=(input,result)=>input.op==="reserve"?{...result,extra:true}:
    input.op==="status"&&f.ops("reserve").length>0?{...result,pending:{...result.pending,operation:null}}:result;
  f.quote();assert.equal(f.state.pending.phase,"queued");assert.equal(f.ops("cancelUnsent").length,0);
  assert.equal(f.ops("enter").length,0);assert(f.ops("withdraw").length>=2);
});
test("Source30 old Gold barrier refuses quote without withdrawing or resetting legacy custody",()=>{
  const f=s30Harness();f.begin();f.state.old=true;assert.equal(f.client.purchase(s30Request,"gold",()=>true),false);
  assert.equal(f.ops("quote").length,0);assert.equal(f.ops("reserve").length,0);
  const page=npcPageHarness();assert.equal(page.dispatcher.hasRetainedAttempt(),true);
  page.dispatcher.observe();assert.equal(page.dispatcher.hasRetainedAttempt(),false);
  for(const phase of ["queued","bound","entered","flushed","unknown"]){page.protocol.hold(phase);assert.equal(page.dispatcher.hasRetainedAttempt(),true,phase);}
  const unavailable=new NpcGoldBuyDispatcher({runtime:{},read:()=>null,getAttemptSlot:()=>null,readSocket:()=>null});
  assert.equal(unavailable.hasRetainedAttempt(),true);
});
test("Source30 dispatcher malformed extra fields or accessor decisions cannot send",()=>{
  for(const edge of ["extra","wrongBody","numericID","accessor"]){const f=s30Harness();assert(f.client.open());
    f.state.mutate=(input,result)=>{if(input.op!=="begin")return result;if(edge==="extra")result.extra=true;
      if(edge==="wrongBody")result.body='{"type":"buyItem"}';if(edge==="numericID")result.request.requestId=1;
      if(edge==="accessor")Object.defineProperty(result,"body",{enumerable:true,get(){assert.fail("decision getter executed");}});return result;};
    assert.equal(f.client.begin(),false,edge);assert.equal(f.sent.length,0,edge);
  }
});
test("Source30 actual Page branches raw owner before generic decoder and complete application bypasses movement overlays",()=>{
  const rawBranch=actualPageSource.indexOf("if (isNpcPurchaseOwnerFrame(event.data))");
  assert(rawBranch>=0&&rawBranch<actualPageSource.indexOf("const decoded = parseGatewayMailDates(event.data"));
  assert(actualPageSource.includes("if (!fullEconomy && isMovementOnly && snapSelf)"));
  assert(actualPageSource.includes("stage5Systems: fullEconomy ? snapshot.stage5Systems! : snapshot.stage5Systems"));
  assert(actualPageSource.includes("mergedEntitiesForWorld = packetRuntimeRefresh && !fullEconomy"));
  assert(actualPageSource.includes("mergedGroundDropsForWorld = packetRuntimeRefresh && !fullEconomy"));
  assert(actualPageSource.includes("applyNpcGoldBuyGatewaySnapshot(snapshot, generation, true)"));
  assert(actualPageSource.includes("npcPurchaseEconomicSourceRef.current = Object.freeze({ socket, session, rawFrame, snapshot: rawSnapshot })"));
  assert(actualPageSource.includes("clientCapabilities\", capabilities: [\"durableNpcPurchaseOwnerV1"));
});
test("Source30 actual Pearl catalogue accepts and preserves only canonical purchase selectors",()=>{
  const f=installPearlPage(npcPageHarness()),physical=f.currentSocialReceiveOwner();
  for(const selector of ["0","18446744073709551615"]){const raw=copy(f.rawPearl);raw.list[0].purchaseItemIndex=selector;
    const catalog=new actualPearlSource.NpcPearlShopSource().observeCatalog(physical,"NPCPearlGoods",raw,()=>true);
    assert(catalog);assert.equal(catalog.rawPayload.list[0].purchaseItemIndex,selector);
  }
  for(const selector of [0,"01","-1","18446744073709551616"]){const raw=copy(f.rawPearl);raw.list[0].purchaseItemIndex=selector;
    assert.equal(new actualPearlSource.NpcPearlShopSource().observeCatalog(physical,"NPCPearlGoods",raw,()=>true),null);
  }
});
test("Source30 terminal frame applies its entire same revision economic source before complete witness",()=>{
  const f=s30Harness();f.begin();assert(f.client.purchase(s30Request,"gold",()=>true));f.quote();
  const snapshot=s30Snapshot();snapshot.gold=66;snapshot.playerExperience=48;snapshot.playerMaxExperience=200;snapshot.entities[0].level=2;
  snapshot.stage5Systems.intelligentCreaturePearls=7;snapshot.stage5Systems.mentor.menteeExp=100;
  snapshot.stage5Systems.relationship.partnerName="Spouse";snapshot.stage5Systems.economyProjectionEventIds=["event-original"];
  snapshot.stage5Systems.mail=[{id:1,from:"Other",to:"Actor",subject:"",body:"Economic mail",gold:10,items:[],opened:false,locked:false,claimed:false,deleted:false}];
  const authority={...s30Authority,serverRevision:"1"},receipt={producerScope:s30Scope,entry:{operation:s30Operation,serverRevision:"1",
    outcome:{committed:{request:s30Request,currency:"gold",source:"trade",charged:34,admittedCount:2,incomingUniqueId:"0"}}}};
  const raw=f.frame(f.sent.at(-1).requestId,{kind:"purchase",receipt,replayed:false},snapshot,authority);
  f.client.receive(raw);assert.equal(f.applied.length,2);assert.equal(f.applied.at(-1).raw,raw);
  const economic=f.applied.at(-1).snapshot;assert.equal(economic.gold,66);assert.equal(economic.playerExperience,48);assert.equal(economic.entities[0].level,2);
  assert.equal(economic.stage5Systems.mail[0].body,"Economic mail");assert.equal(economic.stage5Systems.mentor.menteeExp,100);
  assert.equal(economic.stage5Systems.relationship.partnerName,"Spouse");assert.equal(economic.stage5Systems.intelligentCreaturePearls,7);
  assert.deepEqual([...economic.stage5Systems.economyProjectionEventIds],["event-original"]);assert.equal(f.ops("applied").at(-1).authority.serverRevision,"1");
});
test("Source30 terminal receipt with partial snapshot preserves owner observation without complete or repeat purchase",()=>{
  const f=s30Harness();f.begin();assert(f.client.purchase(s30Request,"gold",()=>true));f.quote();
  const receipt={producerScope:s30Scope,entry:{operation:s30Operation,serverRevision:"1",outcome:{rejected:{request:s30Request,reason:"unknownGood"}}}};
  f.client.receive(f.frame(f.sent.at(-1).requestId,{kind:"purchase",receipt,replayed:false},{inventoryItems:[]},{...s30Authority,serverRevision:"1"}));
  assert.equal(f.ops("receive").length,3);assert.equal(f.ops("applied").length,1);assert.equal(f.ops("reserve").length,1);
  assert.equal(f.sent.filter(row=>row.action.kind==="purchase").length,1);
});
test("Source30 actual owner Pearl projection preserves equal wallet custody and carries canonical selectors on changed balance",()=>{
  const f=installPearlPage(npcPageHarness()),service=f.scope.npcShopServiceRef.current,source=f.scope.npcPearlShopSourceRef.current;
  const selectors=new Map([[0,"0"]]);f.scope.npcPurchaseSelectorsRef.current.set(service,selectors);
  const wallet=source.currentWallet(f.currentSocialReceiveOwner());f.scope.npcPurchaseApplyingEconomyRef.current=true;
  f.observeNpcPearlWallet(200);assert.equal(f.scope.npcShopServiceRef.current,service);assert.equal(source.currentWallet(f.currentSocialReceiveOwner()),wallet);
  f.observeNpcPearlWallet(201);assert.equal(source.currentWallet(f.currentSocialReceiveOwner()).amount,201);
  const next=f.scope.npcShopServiceRef.current;assert.notEqual(next,service);assert.equal(f.scope.npcPurchaseSelectorsRef.current.get(next),selectors);
});

test("Source31 original owner numeric UID MAX and signed i64 dates project only exact declared snapshot leaves",()=>{
  const raw='{"snapshot":{"inventoryItems":[{"uniqueId":18446744073709551615,"tooltipSource":{"userItem":{"unique_id":18446744073709551615,"slots":[{"unique_id":9007199254740993,"slots":[],"sealed_info":{"expiry_binary_datetime":-9223372036854775808,"next_seal_binary_datetime":9223372036854775807}}],"expire_info":{"expiry_binary_datetime":621355968000000001}}}}],"equipmentItems":[{"uniqueId":0,"sealedExpiryTimeBinaryDatetime":9223372036854775807,"sealedNextTimeBinaryDatetime":-9223372036854775808}],"storagePasswordLastSetBinaryDatetime":621355968000000001,"expandedStorageExpiryTimeBinaryDatetime":9223372036854775807,"stage5Systems":{"relationship":{"marriedDateBinaryDatetime":621355968000000001},"mail":[{"dateSentBinaryDatetime":-9223372036854775808}],"itemRental":{"rentedItems":[{"itemId":18446744073709551615,"itemReturnDateBinaryDatetime":9223372036854775807}]}}}}';
  assert.throws(()=>actualDurableNpc.parseNpcPurchaseJson(raw));
  const projected=actualDurableNpc.parseNpcPurchaseJson(raw,true).snapshot,owned=projected.inventoryItems[0];
  assert.equal(owned.uniqueId,"18446744073709551615");assert.equal(owned.tooltipSource.userItem.unique_id,owned.uniqueId);
  assert.equal(owned.tooltipSource.userItem.slots[0].unique_id,"9007199254740993");
  assert.equal(owned.tooltipSource.userItem.slots[0].sealed_info.expiry_binary_datetime,"-9223372036854775808");
  assert.equal(owned.tooltipSource.userItem.slots[0].sealed_info.next_seal_binary_datetime,"9223372036854775807");
  assert.equal(projected.equipmentItems[0].uniqueId,0);assert.equal(projected.stage5Systems.mail[0].dateSentBinaryDatetime,"-9223372036854775808");
  assert.equal(projected.stage5Systems.itemRental.rentedItems[0].itemId,"18446744073709551615");
  assert(Object.isFrozen(projected.inventoryItems[0].tooltipSource.userItem.slots));
});
test("Source31 exact owner integer projection rejects wrong paths duplicate fields fractions and integer range escapes",()=>{
  for(const raw of ['{"uniqueId":18446744073709551615}','{"snapshot":{"unknown":{"uniqueId":18446744073709551615}}}',
    '{"snapshot":{"inventoryItems":[{"uniqueId":18446744073709551616}]}}',
    '{"snapshot":{"inventoryItems":[{"uniqueId":-1}]}}',
    '{"snapshot":{"inventoryItems":[{"uniqueId":9007199254740991.1}]}}',
    '{"snapshot":{"inventoryItems":[{"uniqueId":1e3}]}}',
    '{"snapshot":{"inventoryItems":[{"uniqueId":0,"\\u0075niqueId":1}]}}',
    '{"snapshot":{"storagePasswordLastSetBinaryDatetime":-9223372036854775809}}',
    '{"snapshot":{"expandedStorageExpiryTimeBinaryDatetime":9223372036854775808}}',
    '{"snapshot":{"inventoryItems/*/uniqueId":18446744073709551615}}']){
    assert.throws(()=>actualDurableNpc.parseNpcPurchaseJson(raw,true),undefined,raw);
  }
  for(const value of ["-9223372036854775808","9223372036854775807","0"])assert(actualDurableNpc.npcPurchaseSignedDecimal(value));
  for(const value of ["-0","00","-01","9223372036854775808","-9223372036854775809",1])assert.equal(actualDurableNpc.npcPurchaseSignedDecimal(value),false);
});

// Numeric literals are reconstructed only inside these source fixtures. Product
// projection receives the original string, never a pre-rounded JS object.
const s31Max="18446744073709551615",s31MinDate="-9223372036854775808",s31MaxDate="9223372036854775807";
function s31UserItem(uid=s31Max){return {unique_id:uid,item_index:10,count:1,current_dura:10,max_dura:10,
  soul_bound_id:0,identified:true,cursed:false,slots:[],gem_count:0,added_stats:[],awake_type:0,awake_values:[],
  refined_value:0,refine_added:0,refine_success_chance:0,wedding_ring:0,expire_info:{expiry_binary_datetime:s31MinDate},
  rental_information:null,is_shop_item:false,sealed_info:{expiry_binary_datetime:s31MaxDate,next_seal_binary_datetime:s31MinDate},gm_made:false};}
function s31ItemInfo(){return {item_index:10,name:"Exact item",item_type:13,grade:0,required_type:0,required_class:31,
  required_gender:3,item_set:0,shape:0,weight:1,light:0,required_amount:1,image:1,durability:10,stack_size:20,price:1,
  start_item:false,effect:0,need_identify:false,show_group_pickup:false,class_based:false,level_based:false,can_mine:false,
  global_drop_notify:false,bind:0,unique:0,random_stats_id:0,can_fast_run:false,can_awakening:false,slots:0,stats:[],tooltip:null};}
function s31WorldItem(uid=s31Max,slot=0,container="bag1"){return {key:"exact",name:"Exact item",description:"Original",
  icon:1,uniqueId:uid,slot,container,quantity:1,tooltipSource:{info:s31ItemInfo(),userItem:s31UserItem(uid)}};}
function s31Shop(uid=s31Max,service="BUYUSED",count=1,npcObjectId=50){
  const item=s31UserItem(uid);item.count=count;item.is_shop_item=!["BUYUSED","BUYBACK"].includes(service);
  if(typeof uid==="number"){item.expire_info=null;item.sealed_info=null;}
  const info=s31ItemInfo();
  return {npcObjectId,scriptKey:"Bichon/Shop",service,packetType:service==="PEARLBUY"?"NPCPearlGoods":"NPCGoods",
    list:[item],displayGoods:[{...copy(item),id:uid,uniqueId:uid,purchaseItemIndex:String(uid),itemIndex:item.item_index,
      name:info.name,icon:info.image,price:info.price,grade:info.grade,
      tooltipSource:{info,realInfo:null,userItem:copy(item),socketInfos:[],realSocketInfos:[]}}],
    rate:1,panelType:service==="BUYUSED"?1:0,hideAddedStats:false};
}
function s31Snapshot(){const v=s30Snapshot();v.heroMaxExperience=null;v.inventoryItems=[s31WorldItem()];
  v.storageItems=[s31WorldItem("9007199254740993",0,"storage")];
  v.equipmentItems=[{...s31WorldItem("9007199254740995"),slot:"weapon",sealedExpiryTimeBinaryDatetime:s31MaxDate,sealedNextTimeBinaryDatetime:s31MinDate}];delete v.equipmentItems[0].container;
  v.storagePasswordLastSetBinaryDatetime=s31MinDate;v.expandedStorageExpiryTimeBinaryDatetime=s31MaxDate;
  v.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:[s31Max]};
  v.nativeNpcShop=s31Shop();v.entities.push({objectId:50,name:"Actual shop",x:11,y:10,direction:"Down",dead:false});
  v.stage5Systems.relationship.marriedDateBinaryDatetime=s31MinDate;
  v.stage5Systems.mail=[{id:1,from:"Other",to:"Actor",subject:"Exact",body:"Saved item",gold:0,items:["exact"],
    itemStatesJson:['{"unique_id":18446744073709551615,"key":"exact","name":"Exact item","quantity":1,"sealed_expiry_time_binary_datetime":-9223372036854775808,"socketed":[{"unique_id":9007199254740993,"key":"socket","name":"Socket","quantity":1}],"user_item_metadata":{"item_index":10,"rental_information":{"expiry_binary_datetime":9223372036854775807},"slots":[]}}'],opened:false,locked:false,claimed:false,deleted:false}];
  return v;}
function s31OriginalFrame(f,snapshot=s31Snapshot()){return f.frame(f.sent.at(-1).requestId,{kind:"producer",producer:s30Authority},snapshot,s30Authority)
  .replace(/("(?:uniqueId|unique_id|id|sealedExpiryTimeBinaryDatetime|sealedNextTimeBinaryDatetime|storagePasswordLastSetBinaryDatetime|expandedStorageExpiryTimeBinaryDatetime|marriedDateBinaryDatetime|expiry_binary_datetime|next_seal_binary_datetime)":)"(18446744073709551615|9007199254740993|9007199254740995|-9223372036854775808|9223372036854775807)"/g,"$1$2")
  .replace(/("freshCompatibleUniqueIds":\[)"18446744073709551615"/g,(_,prefix)=>prefix+s31Max);}

test("Source31 actual textual receipt facade preserves raw ABI snapshot and keeps control integers strict",()=>{
  const f=liveReceiptFacade(),raw='{"ok":true,"frame":{"snapshot":{"nativeNpcShop":{"list":[{"unique_id":18446744073709551615,"sealed_info":{"expiry_binary_datetime":-9223372036854775808}}]},"storagePasswordLastSetBinaryDatetime":9223372036854775807}}}';
  f.oracle.responses.set("receive",raw);const value=f.runtime.transact(f.source,f.socket,{op:"receive",frame:"original owner"});
  assert.equal(value.frame.snapshot.nativeNpcShop.list[0].unique_id,s31Max);assert.equal(value.frame.snapshot.storagePasswordLastSetBinaryDatetime,s31MaxDate);
  assert(Object.isFrozen(value.frame.snapshot.nativeNpcShop.list[0]));
  for(const invalid of ['{"ok":true,"sequence":18446744073709551615}','{"ok":true,"frame":{"snapshot":{"unknown":{"uniqueId":18446744073709551615}}}}','{"ok":true,"ok":false}']){
    const bad=liveReceiptFacade();bad.oracle.responses.set("receive",invalid);assert.equal(bad.runtime.transact(bad.source,bad.socket,{op:"receive",frame:"raw"}).ok,false);
  }
});
test("Source31 raw full owner agrees with new ABI and quarantined old host before one complete witness",()=>{
  for(const old of [true,false]){const f=s30Harness();assert(f.client.open());assert(f.client.begin());const raw=s31OriginalFrame(f);
    if(!old)f.state.mutate=(input,result)=>input.op==="receive"?actualDurableNpc.parseNpcPurchaseJson('{"ok":true,"frame":'+input.frame+',"observation":{"kind":"ignored"},"kind":"producer","binding":'+JSON.stringify(f.state.binding)+'}',"abi"):result;
    f.client.receive(raw);assert.equal(f.applied.length,1);assert.equal(f.ops("applied").length,1);
    const source=f.applied[0].snapshot;assert.equal(source.inventoryItems[0].uniqueId,s31Max);assert.equal(source.equipmentItems[0].sealedNextTimeBinaryDatetime,s31MinDate);
    assert.equal(source.storageItems[0].uniqueId,"9007199254740993");assert.equal(source.nativeNpcShop.list[0].unique_id,s31Max);
    assert.equal(f.applied[0].raw,raw);assert.equal(source.stage5Systems.mail[0].itemStatesJson[0],s31Snapshot().stage5Systems.mail[0].itemStatesJson[0]);
    f.client.receive(raw);assert.equal(f.applied.length,1);assert.equal(f.ops("applied").length,1);
  }
});
test("Source31 old host quarantine rejects changed receipt identity shape accessor and reflection reentry",()=>{
  for(const edge of ["identity","wallet","key","getter","receipt","reentry"]){const f=s30Harness();assert(f.client.open());assert(f.client.begin());let getter=0;
    f.state.mutate=(input,result)=>{if(input.op!=="receive")return result;
      if(edge==="identity")result.frame.snapshot.inventoryItems[0].uniqueId=Number("18446744073709547520");
      if(edge==="wallet")result.frame.snapshot.gold++;
      if(edge==="key")result.frame.snapshot.inventoryItems[0].extra=true;
      if(edge==="receipt")result.frame.authority.serverRevision="1";
      if(edge==="getter")Object.defineProperty(result.frame.snapshot.inventoryItems[0],"uniqueId",{enumerable:true,get(){getter++;return Number(s31Max);}});
      if(edge==="reentry")result.frame.snapshot=new Proxy(result.frame.snapshot,{ownKeys(target){f.client.withdraw();return Reflect.ownKeys(target);}});
      return result;};
    f.client.receive(s31OriginalFrame(f));assert.equal(f.applied.length,0,edge);assert.equal(f.ops("applied").length,0,edge);assert.equal(getter,0);
  }
});
test("Source31 exact bag identities and capacity roster compare canonically without numeric mutation authority",()=>{
  const v=actualDurableNpc.parseNpcPurchaseJson('{"snapshot":'+JSON.stringify(s31Snapshot())+'}',true).snapshot;
  const source={...v,inventoryItems:v.inventoryItems.map(row=>({...row,...expiryIdentity.projectInventoryItemIdentity(row.uniqueId,row.container,row.slot)})),
    equipmentItems:v.equipmentItems.map(row=>({...row,uniqueId:undefined,exactUniqueId:row.uniqueId})),beltItems:[]};
  const result=expiryBag.projectBevyBagModel(source,{npcGoldTrade:true});assert(result.ok);assert.equal(result.model.items[0].exactUniqueId,s31Max);
  assert.equal(result.model.items[0].uniqueId,null);assert.equal(expiryIdentity.authoritativeItemUniqueId(s31Max),undefined);
  const owner={connectionGeneration:1,sessionGeneration:1,playerObjectId:7},ready=new module.exports.NpcGoldBuyInventoryReadiness();
  assert(ready.finish(ready.begin(owner),owner,true,result.model));assert(ready.matches(owner,result.model));
  source.inventoryItems.push({...source.inventoryItems[0],slot:1});assert.equal(expiryBag.projectBevyBagModel(source,{npcGoldTrade:true}).ok,false);
  source.inventoryItems.pop();source.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:[1,"1"]};assert.equal(expiryBag.projectBevyBagModel(source,{npcGoldTrade:true}).ok,false);
  source.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:["9007199254740993"]};assert.equal(expiryBag.projectBevyBagModel(source,{npcGoldTrade:true}).ok,false);
});
test("Source31 saved mail item JSON and signed dates remain exact and reject duplicate or rounded attachments",()=>{
  const mail=s31Snapshot().stage5Systems.mail;mail[0].dateSentBinaryDatetime=s31MinDate;mail[0].canReply=true;
  const parsed=expiryPackets.parseMailList(mail);assert(parsed);assert.equal(parsed[0].items[0].uniqueId,s31Max);assert.equal(parsed[0].dateSentBinaryDatetime,s31MinDate);
  const saved=actualDurableNpc.parseNpcPurchaseMailItemJson(mail[0].itemStatesJson[0]);assert.equal(saved.unique_id,s31Max);
  assert.equal(saved.socketed[0].unique_id,"9007199254740993");assert.equal(saved.sealed_expiry_time_binary_datetime,s31MinDate);
  assert.equal(saved.user_item_metadata.rental_information.expiry_binary_datetime,s31MaxDate);
  const bad=copy(mail);bad[0].itemStatesJson.push(bad[0].itemStatesJson[0]);assert.equal(expiryPackets.parseMailList(bad),null);
  bad[0].itemStatesJson=['{"unique_id":0,"unique_id":18446744073709551615}'];assert.equal(expiryPackets.parseMailList(bad),null);
  for(const value of [Number(s31Max),"18446744073709551616"]){const direct=copy(mail);delete direct[0].itemStatesJson;direct[0].items=[{unique_id:value,count:1}];assert.equal(expiryPackets.parseMailList(direct),null);}
  for(const date of ["-0","01",Number(s31MaxDate)]){const direct=copy(mail);direct[0].dateSentBinaryDatetime=date;assert.equal(expiryPackets.parseMailList(direct)[0].metadataKnown,false);}
});
const s31Hero=loadPearlBuyDependency("../lib/hero-player-ui.ts");
function s31HeroFixture(){const owner={connectionGeneration:1,sessionGeneration:7,playerObjectId:40,socket:{},sceneRevision:2,mapFileName:"0"};
  const info={object_id:70,name:"Hero",class:"Warrior",gender:"Male",level:20,hair:0,hp:100,mp:50,experience:1,max_experience:100,
    inventory:Array(10).fill(null),equipment:Array(14).fill(null),magics:[],auto_pot:true,auto_hp_percent:40,auto_mp_percent:30,hp_item_index:10,mp_item_index:0};
  const world={playerObjectId:40,mapFileName:"0",inventoryCapacity:46,maxBagSlots:40,inventoryItems:[],heroInventoryCapacity:10,heroInventoryItems:[],heroEquipmentItems:[],
    heroStats:[],heroVitals:{hp:100,mp:50},heroWeights:{bag:0,wear:0,hand:0},heroMaxExperience:1000,
    stage5Systems:{hero:{name:"Hero",class:"Warrior",gender:"Male",level:20,spawned:true,autoPot:true,autoHpPercent:40,autoMpPercent:30,hpItemIndex:10,mpItemIndex:0,experience:55},heroLearnedMagics:[]}};
  const authority=new s31Hero.HeroPlayerAuthority();assert(authority.receiveInformation(info,owner));assert(authority.receiveSnapshot(world,owner));return {owner,info,world,authority};}
test("Source31 Hero reads same snapshot experience and actual max pair instead of cached packet values",()=>{
  const f=s31HeroFixture();assert.equal(f.authority.read(f.owner).experience,55);assert.equal(f.authority.read(f.owner).maxExperience,1000);
  f.world.stage5Systems.hero.experience="9007199254740993";f.world.heroMaxExperience=s31MaxDate;assert(f.authority.receiveSnapshot(f.world,f.owner));
  assert.equal(f.authority.read(f.owner).experience,"9007199254740993");assert.equal(f.authority.read(f.owner).maxExperience,s31MaxDate);
  assert(f.authority.receiveInformation(f.info,f.owner));assert.equal(f.authority.read(f.owner).experience,1);
  assert(f.authority.receiveSnapshot(f.world,f.owner));assert.equal(f.authority.read(f.owner).experience,"9007199254740993");
  for(const missing of [null,Number(s31MaxDate),"9223372036854775808"]){f.world.heroMaxExperience=missing;assert(f.authority.receiveSnapshot(f.world,f.owner));assert.equal(f.authority.read(f.owner),null);}
  delete f.world.heroMaxExperience;assert(f.authority.receiveSnapshot(f.world,f.owner));assert.equal(f.authority.read(f.owner).experience,1);
});
test("Source31 Hero canonical full UID roster remains exact and cross-grid duplicates deny the model",()=>{
  const f=s31HeroFixture();f.world.heroInventoryItems=[s31WorldItem()];assert(f.authority.receiveSnapshot(f.world,f.owner));
  const model=f.authority.read(f.owner);assert(model);assert.equal(model.inventory[0].uniqueId,s31Max);assert.equal(model.inventory[0].userItem.unique_id,s31Max);
  assert.equal(s31Hero.planHeroAction(model,{kind:"use",slot:0}),null);assert.equal(s31Hero.heroRestockCandidate(model,0),null);
  f.world.heroEquipmentItems=[s31WorldItem(s31Max,3)];assert(f.authority.receiveSnapshot(f.world,f.owner));assert.equal(f.authority.read(f.owner),null);
  f.world.heroEquipmentItems=[];f.world.heroInventoryItems[0].uniqueId=Number(s31Max);assert(f.authority.receiveSnapshot(f.world,f.owner));assert.equal(f.authority.read(f.owner),null);
});
test("Source31 complete Hero owner requires actual max pair and validates exact new shop integer leaves",()=>{
  const v=s31Snapshot();v.stage5Systems.hero={experience:55};delete v.heroMaxExperience;assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false);
  v.heroMaxExperience=1000;assert(actualDurableNpc.completeNpcPurchaseSnapshot(v));v.heroMaxExperience=null;assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false);
  v.stage5Systems.hero=null;assert(actualDurableNpc.completeNpcPurchaseSnapshot(v));v.nativeNpcShop.list[0].unique_id="18446744073709551616";assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(v),false);
});
test("Source31 actual Page durable full UID baseline preserves native shop with null main dialog across reads",()=>{
  const f=npcPageHarness(),w=f.scope.worldRef.current;w.stage5Systems={hero:null};w.credit=0;w.activeNpcDialog=null;
  w.inventoryItems=[{...s31WorldItem(),...expiryIdentity.projectInventoryItemIdentity(s31Max,"bag1",0)}];
  w.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:[s31Max]};
  const snapshot={...w,playerObjectId:1,nativeNpcShop:s31Shop(77,"BUYSELL")};
  snapshot.entities=[...snapshot.entities,{objectId:50,name:"Actual shop"}];
  f.scope.npcPurchaseApplyingEconomyRef.current=true;f.applyNpcPurchaseShopSnapshot(snapshot);f.scope.npcPurchaseApplyingEconomyRef.current=false;
  f.scope.npcPurchaseEconomicSourceRef.current={socket:f.socket,session:f.npcPurchaseSessionKey(),snapshot,rawFrame:"strict original fixture"};
  f.scope.npcPurchaseDisplaySourceRef.current={...f.scope.npcPurchaseEconomicSourceRef.current,fingerprint:f.npcPurchaseDisplayFingerprint(w)};
  f.scope.equipmentControllerRef.current.status=()=>({ready:false,pending:0});
  const owner={connectionGeneration:1,sessionGeneration:2,playerObjectId:1},model=npcPureBag.projectBevyBagModel(w,{npcGoldTrade:true});assert(model.ok);
  assert(f.scope.npcGoldBuyInventoryRef.current.finish(f.scope.npcGoldBuyInventoryRef.current.begin(owner),owner,true,model.model));
  const service=f.scope.npcShopServiceRef.current;
  for(let i=0;i<2;i++){const current=f.readNpcGoldBuyCurrent();assert(current);assert.equal(current.blocked,false);assert.equal(current.inventory.items[0].exactUniqueId,s31Max);assert.equal(f.scope.npcShopServiceRef.current,service);}
  f.scope.equipmentControllerRef.current.status=()=>({ready:false,pending:1});assert.equal(f.readNpcGoldBuyCurrent().blocked,true);
  f.scope.equipmentControllerRef.current.status=()=>({ready:false,pending:0});f.scope.npcGoldBuyInventoryRef.current.invalidate();assert.equal(f.readNpcGoldBuyCurrent().blocked,true);
  const valid=s31Snapshot();assert.equal(valid.activeNpcDialog,null);assert(actualDurableNpc.completeNpcPurchaseSnapshot(valid));
  valid.activeNpcDialog={npcObjectId:51};assert.equal(actualDurableNpc.completeNpcPurchaseSnapshot(valid),false);
});
test("Source31 ordinary declared storage dates reuse exact signed codec and unsupported numeric source is unavailable",()=>{
  const raw='{"type":"packet","packet":"ResizeStorage","payload":{"hasExpandedStorage":true,"size":160,"expiryTimeBinaryDatetime":9223372036854775807}}';
  const supported=expiryPackets.parseGatewayMailDates(raw);assert.equal(supported.payload.expiryTimeBinaryDatetime,s31MaxDate);
  // This injected parser deliberately provides no reviver token context.
  const unsupported=loadExpiryPipelineModule("../lib/extended-server-packets.ts",{"./npc-purchase-client":actualDurableNpc},
    {parse(text,reviver){return JSON.parse(text,reviver?function(key,value){return reviver.call(this,key,value);}:undefined);},stringify:JSON.stringify});
  assert.equal(unsupported.parseGatewayMailDates(raw).payload.expiryTimeBinaryDatetime,null);
  for(const value of [Number(s31MaxDate),"-0","9223372036854775808"])assert.equal(expiryPackets.exactGatewayBinaryDatetime(value),undefined);
  assert.equal(expiryPackets.exactGatewayBinaryDatetime(s31MinDate),s31MinDate);assert.equal(expiryPackets.exactGatewayBinaryDatetime(0),0);
  assert(actualPageSource.includes("exactGatewayBinaryDatetime(payload.lastSetBinaryDatetime)"));
});
// Execute the real Page socket/client constructor, owner apply callback, catalog
// reducers, current readers and final durable dispatcher. Only the existing
// world-model boundary and Core ABI are fixtures; this does not start WASM/UI.
function s31PageOwner(service="BUYUSED"){
  const page=npcPageHarness(),abi=s30Harness();
  const core={...page.scope.questCoreRuntimeRef.current,getNpcPurchaseReceiptHost:()=>abi.host,
    getMir2NpcPearlBuyPlan:json=>JSON.stringify({version:1,ok:true,maxQuantity:99,quote:JSON.parse(json).unitPrice,admittedCount:null,denial:4})};
  page.scope.questCoreRuntimeRef.current=core;page.dispatcher.observe();page.observeNpcPurchaseSocket(page.socket);
  const client=page.scope.npcPurchaseClientRef.current?.client;assert(client);
  const ownerWires=()=>page.sent.filter(value=>value.type==="npcPurchaseOwner");
  const snapshot=s30Snapshot();snapshot.heroMaxExperience=null;snapshot.nativeNpcShop=s31Shop(77,service,3);
  snapshot.entities.push({objectId:50,name:"Actual shop",x:11,y:10,direction:"Down",dead:false});
  snapshot.inventoryItems=[s31WorldItem()];snapshot.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:[s31Max]};
  snapshot.stage5Systems.intelligentCreaturePearls=100;
  const raw=(reply,view=null,authority=null,id=ownerWires().at(-1).requestId)=>s31OriginalFrame({
    sent:[{requestId:id}],frame:(requestId,_reply,_snapshot,_authority)=>abi.frame(requestId,reply,view,authority)},view);
  const begin=view=>client.receive(raw({kind:"producer",producer:s30Authority},view,s30Authority));
  begin(snapshot);assert.equal(abi.ops("applied").length,1);assert(page.scope.npcShopServiceRef.current);
  const quote=(view=null)=>{
    const wire=ownerWires().at(-1);assert.equal(wire.action.kind,"quote");
    abi.state.intent={...copy(s30Intent),request:wire.action.request,currency:service==="PEARLBUY"?"pearls":"gold",
      source:service==="BUYUSED"?"used":service==="BUYBACK"?"buyBack":"trade"};
    abi.state.operation={...copy(s30Operation),sequence:String(abi.ops("reserve").length+1),intent:abi.state.intent};
    client.receive(raw({kind:"quote",intent:abi.state.intent},view,view?s30Authority:null,wire.requestId));
  };
  return {page,abi,client,snapshot,ownerWires,raw,begin,quote};
}
test("Source31 actual Page owner apply refreshes Gold Pearl BuyBack Used catalog before first and second purchase",()=>{
  for(const profile of ["BUYSELL","PEARLBUY","BUYBACK","BUYUSED"]){
    const f=s31PageOwner(profile),p=f.page;let snapshot=f.snapshot;
    for(let turn=0;turn<2;turn++){
      const service=p.scope.npcShopServiceRef.current,id=service.buyItems[0].id;
      assert.equal(p.scope.worldRef.current.activeNpcDialog,null);assert.equal(service.buyItems[0].count,3-turn);
      assert.equal(service.buyItems[0].name,snapshot.nativeNpcShop.displayGoods[0].name);
      for(let read=0;read<2;read++){assert.equal(p.readNpcBuyCurrent().blocked,false);assert.equal(p.scope.npcShopServiceRef.current,service);}
      p.scope.npcBuySelectedRef.current=id;assert(p.dispatchDurableNpcPurchase(id,1));
      f.quote(snapshot);assert.equal(p.scope.npcShopServiceRef.current,service,"same Quote source preserves UI custody");
      const entered=f.ownerWires().at(-1);assert.equal(entered.action.kind,"purchase");assert.equal(entered.action.operation.intent.request.itemIndex,String(id));
      assert.equal(entered.action.operation.intent.currency,profile==="PEARLBUY"?"pearls":"gold");
      const next=copy(snapshot);next.nativeNpcShop=s31Shop(78+turn,profile,2-turn);
      next.inventoryItems.push(s31WorldItem(10001+turn,1+turn));next.npcGoldTradeCapacity.freshCompatibleUniqueIds.push(10001+turn);
      const authority={...s30Authority,serverRevision:String(turn+1)};
      const receipt={producerScope:s30Scope,entry:{operation:entered.action.operation,serverRevision:String(turn+1),outcome:{committed:{
        request:entered.action.operation.intent.request,currency:entered.action.operation.intent.currency,source:entered.action.operation.intent.source,
        charged:1,admittedCount:1,incomingUniqueId:String(10001+turn)}}}};
      f.abi.state.mutate=(input,result)=>{if(input.op==="applied")f.abi.state.pending=null;return result;};
      f.client.receive(f.raw({kind:"purchase",receipt,replayed:false},next,authority,entered.requestId));
      assert.equal(f.abi.state.pending,null);assert.notEqual(p.scope.npcShopServiceRef.current,service);
      assert.equal(p.scope.npcBuySelectedRef.current,null);assert.equal(p.scope.npcShopServiceRef.current.buyItems[0].id,78+turn);
      assert.equal(p.readNpcBuyCurrent().blocked,false);snapshot=next;
    }
    assert.equal(f.abi.ops("reserve").length,2);assert.equal(f.abi.ops("enter").length,2);
    assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,2);assert.equal(f.abi.ops("begin").length,1);
    assert.equal(p.sent.some(w=>w.type==="buyItem"),false);assert.equal(f.abi.ops("applied").length,5);
  }
});
test("Source31 actual Page catalog switches NPC and withdraws missing unknown or unsupported full source",()=>{
  const f=s31PageOwner(),p=f.page,original=p.scope.npcShopServiceRef.current;
  p.scope.npcBuySelectedRef.current=77;const next=copy(f.snapshot);next.nativeNpcShop=s31Shop(78,"BUYUSED",2,51);
  next.entities.push({objectId:51,name:"Second actual NPC",x:12,y:10,direction:"Down",dead:false});
  p.scope.npcPurchaseApplyingEconomyRef.current=true;p.applyNpcPurchaseShopSnapshot(next);p.scope.npcPurchaseApplyingEconomyRef.current=false;
  assert.notEqual(p.scope.npcShopServiceRef.current,original);assert.equal(p.scope.npcShopServiceRef.current.npcName,"Second actual NPC");
  assert.equal(p.scope.npcBuySelectedRef.current,null);assert.equal(p.scope.npcPurchaseCatalogSourceRef.current.get(p.scope.npcShopServiceRef.current).npcObjectId,51);
  for(const kind of ["null","missing","missingDisplay","unknownInfo","maxOffer","missingNpc"]){
    p.scope.npcPurchaseApplyingEconomyRef.current=true;p.applyNpcPurchaseShopSnapshot(f.snapshot);
    const bad=copy(f.snapshot);if(kind==="null")bad.nativeNpcShop=null;if(kind==="missing")delete bad.nativeNpcShop;
    if(kind==="missingDisplay")delete bad.nativeNpcShop.displayGoods;
    if(kind==="unknownInfo")delete bad.nativeNpcShop.displayGoods[0].tooltipSource;
    if(kind==="maxOffer")bad.nativeNpcShop=s31Shop();if(kind==="missingNpc")bad.entities=bad.entities.filter(row=>row.objectId!==50);
    p.applyNpcPurchaseShopSnapshot(bad);p.scope.npcPurchaseApplyingEconomyRef.current=false;
    assert.equal(p.scope.npcShopServiceRef.current,null,kind);assert.equal(p.dispatchDurableNpcPurchase(77,1),false,kind);
    const received=s31PageOwner();assert(received.client.begin(true));received.begin(bad);
    assert.equal(received.abi.ops("applied").length,2,"unavailable catalog still applies complete economy: "+kind);
    assert.equal(received.page.scope.npcShopServiceRef.current,null,kind);
  }
  assert.equal(f.abi.ops("begin").length,1);
});
test("Source31 native display qualification rejects mismatched raw item selector profile duplicate and getter",()=>{
  assert(actualDurableNpc.npcPurchaseShopSource(s31Shop()));
  for(const [profile,panel] of [["BUYBACK",0],["BUYUSED",1]]){
    const actual=s31Shop(77,profile);assert.equal(actual.panelType,panel);assert(actualDurableNpc.npcPurchaseShopSource(actual));
    actual.panelType=1-panel;assert.equal(actualDurableNpc.npcPurchaseShopSource(actual),null);
  }
  for(const edge of ["raw","selector","profile","grade","unknown","duplicate","getter"]){
    const shop=s31Shop(77);let calls=0;
    if(edge==="raw")shop.displayGoods[0].tooltipSource.userItem.count++;
    if(edge==="selector")shop.displayGoods[0].purchaseItemIndex="78";
    if(edge==="profile")shop.packetType="NPCPearlGoods";
    if(edge==="grade")shop.displayGoods[0].grade++;
    if(edge==="unknown")shop.displayGoods[0].stock=50;
    if(edge==="duplicate"){shop.list.push(copy(shop.list[0]));shop.displayGoods.push(copy(shop.displayGoods[0]));}
    if(edge==="getter")Object.defineProperty(shop.displayGoods[0],"id",{enumerable:true,get(){calls++;return 77;}});
    assert.equal(actualDurableNpc.npcPurchaseShopSource(shop),null,edge);assert.equal(calls,0,edge);
  }
  const full=s31Snapshot(),raw=s31OriginalFrame({sent:[{requestId:"1"}],frame:(id,reply,snapshot,authority)=>JSON.stringify({type:"npcPurchaseOwner",protocolVersion:1,requestId:id,reply,snapshot,authority})},full);
  const exact=actualDurableNpc.parseNpcPurchaseJson(raw,true).snapshot;
  assert.equal(exact.nativeNpcShop.displayGoods[0].id,s31Max);assert.equal(exact.nativeNpcShop.displayGoods[0].purchaseItemIndex,s31Max);
  assert.equal(exact.nativeNpcShop.displayGoods[0].tooltipSource.userItem.sealed_info.next_seal_binary_datetime,s31MinDate);
});
test("Source31 real packet refresh Begin preserves entered operation and repeated owner display never loops",()=>{
  const f=s31PageOwner(),p=f.page,source=p.scope.npcPurchaseCatalogSourceRef.current.get(p.scope.npcShopServiceRef.current);
  const samePacket={list:source.displayGoods,rate:source.rate,panelType:source.panelType,hideAddedStats:source.hideAddedStats};
  for(let i=0;i<3;i++)p.applyNpcShopCatalogPacket("NPCGoods",samePacket);
  assert.equal(f.abi.ops("begin").length,1);
  p.scope.npcBuySelectedRef.current=77;assert(p.dispatchDurableNpcPurchase(77,1));f.quote();
  const operation=copy(f.abi.state.pending.operation),changed=s31Shop(78,"BUYUSED",2);
  p.applyNpcShopCatalogPacket("NPCGoods",{list:changed.displayGoods,rate:changed.rate,panelType:changed.panelType,hideAddedStats:changed.hideAddedStats});
  assert.equal(f.abi.ops("begin").length,2);assert.deepEqual(f.abi.state.pending.operation,operation);assert.equal(f.abi.state.pending.phase,"entered");
  const fresh=copy(f.snapshot);fresh.nativeNpcShop=changed;f.begin(fresh);
  assert.equal(f.ownerWires().at(-1).action.kind,"query");assert.deepEqual(f.ownerWires().at(-1).action.operation,operation);
  assert.equal(f.abi.ops("reserve").length,1);assert.equal(f.abi.ops("enter").length,1);
  assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,1);
});
test("Source31 late catalog change after Quote or Enter cannot send a stale gesture and retains original Unknown",()=>{
  for(const edge of ["quote","enter"]){const f=s31PageOwner(),p=f.page;
    p.scope.npcBuySelectedRef.current=77;assert(p.dispatchDurableNpcPurchase(77,1));
    const change=()=>{const next=copy(f.snapshot);next.nativeNpcShop=s31Shop(78,"BUYUSED",2);
      p.scope.npcPurchaseApplyingEconomyRef.current=true;p.applyNpcPurchaseShopSnapshot(next);p.scope.npcPurchaseApplyingEconomyRef.current=false;};
    if(edge==="quote")change();else f.abi.state.mutate=(input,result)=>{if(input.op==="enter")change();return result;};
    f.quote();assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,0,edge);
    assert.equal(f.abi.ops("reserve").length,edge==="quote"?0:1,edge);
    if(edge==="enter"){assert.equal(f.abi.state.pending.phase,"unknown");assert.deepEqual(f.abi.ops("unknown")[0].operation,f.abi.state.pending.operation);}
  }
});
test("Source31 packet refresh final fences reject reentrant connection change or service withdrawal",()=>{
  for(const edge of ["connection","service"]){const f=s31PageOwner(),p=f.page,source=s31Shop(78,"BUYUSED",2);
    const originalSet=p.scope.setNpcShopService;
    // The actual setter argument is captured in the extracted Page closure;
    // use its ordinary dispatcher observer boundary for a synchronous callback.
    const originalObserve=p.dispatcher.observe.bind(p.dispatcher);
    p.dispatcher.observe=()=>{if(edge==="connection")p.scope.equipmentConnectionGenerationRef.current++;
      else p.scope.npcShopServiceRef.current=null;return originalObserve();};
    p.applyNpcShopCatalogPacket("NPCGoods",{list:source.displayGoods,rate:source.rate,panelType:source.panelType,hideAddedStats:false});
    assert.equal(f.abi.ops("begin").length,1,edge);assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,0,edge);
    assert.equal(p.scope.setNpcShopService,originalSet);
  }
});
function s31OrdinaryRaw(snapshot){
  return s31OriginalFrame({sent:[{requestId:"1"}],frame:(_id,_reply,view)=>JSON.stringify({type:"worldSnapshot",payload:view})},snapshot)
    .replace(/("dateSentBinaryDatetime":)"(-9223372036854775808|9223372036854775807)"/g,"$1$2");
}
function s31ReceiveOrdinary(f,snapshot){
  const raw=s31OrdinaryRaw(snapshot);
  f.page.receiveNpcPurchaseGatewayFrame({data:raw},f.page.scope.equipmentConnectionGenerationRef.current,f.page.socket);
  return raw;
}

test("Source31 ordinary raw world preserves full UID signed dates equipment mail and native catalog before application",()=>{
  const f=s31PageOwner("BUYSELL"),p=f.page,paired=p.scope.npcPurchaseEconomicSourceRef.current;
  const snapshot=s31Snapshot();snapshot.nativeNpcShop=s31Shop(77,"BUYSELL",3);
  snapshot.stage5Systems.mail[0].dateSentBinaryDatetime=s31MinDate;
  const applied=f.abi.ops("applied").length,raw=s31ReceiveOrdinary(f,snapshot);
  const exact=p.scope.npcPurchaseDisplaySourceRef.current;
  assert.equal(exact.rawFrame,raw);assert.equal(exact.snapshot.inventoryItems[0].uniqueId,s31Max);
  assert.equal(exact.snapshot.storageItems[0].uniqueId,"9007199254740993");
  assert.equal(exact.snapshot.equipmentItems[0].uniqueId,"9007199254740995");
  assert.equal(exact.snapshot.equipmentItems[0].sealedNextTimeBinaryDatetime,s31MinDate);
  assert.equal(exact.snapshot.storagePasswordLastSetBinaryDatetime,s31MinDate);
  assert.equal(exact.snapshot.expandedStorageExpiryTimeBinaryDatetime,s31MaxDate);
  assert.equal(exact.snapshot.stage5Systems.mail[0].dateSentBinaryDatetime,s31MinDate);
  const saved=actualDurableNpc.parseNpcPurchaseMailItemJson(exact.snapshot.stage5Systems.mail[0].itemStatesJson[0]);
  assert.equal(saved.unique_id,s31Max);assert.equal(saved.socketed[0].unique_id,"9007199254740993");
  assert.equal(saved.user_item_metadata.rental_information.expiry_binary_datetime,s31MaxDate);
  assert(Object.isFrozen(exact.snapshot));const projectedItem=p.scope.worldRef.current.inventoryItems[0];
  assert.equal(projectedItem.exactUniqueId,s31Max);assert.equal(projectedItem.authoritativeUniqueId,undefined);
  assert.equal(expiryIdentity.currentAuthoritativeItem([projectedItem],{container:projectedItem.container,slot:projectedItem.slot,uniqueId:projectedItem.uniqueId}),null);
  assert.equal(expiryIdentity.currentAuthoritativeItem([projectedItem],{container:projectedItem.container,slot:projectedItem.slot,uniqueId:Number(s31Max)}),null);
  const projectedBag=npcPureBag.projectBevyBagModel(p.scope.worldRef.current,{npcGoldTrade:true});assert(projectedBag.ok);
  assert.equal(projectedBag.model.items[0].exactUniqueId,s31Max);assert.equal(projectedBag.model.items[0].uniqueId,null);
  assert.equal(p.readNpcBuyCurrent().blocked,false);assert.equal(p.scope.npcPurchaseEconomicSourceRef.current,paired);
  assert.equal(f.abi.ops("applied").length,applied);assert.equal(f.abi.ops("begin").length,1);
  for(const invalid of [raw.replace('"gold":100','"gold":18446744073709551615'),
    raw.replace('"tick":0','"tick":9007199254740993'),raw.replace('"type":"worldSnapshot"','"type":"worldSnapshot","type":"worldSnapshot"')])
    assert.throws(()=>actualDurableNpc.parseNpcPurchaseWorldFrame(invalid));
});

test("Source31 actual Page equal periodic source and HP XP full fallback preserve first and second buy custody",()=>{
  for(const profile of ["BUYSELL","PEARLBUY","BUYBACK","BUYUSED"]){
    const f=s31PageOwner(profile),p=f.page;let snapshot=copy(f.snapshot);
    for(let turn=0;turn<2;turn++){
      const service=p.scope.npcShopServiceRef.current,paired=p.scope.npcPurchaseEconomicSourceRef.current;
      snapshot.tick+=1;snapshot.entities[0].x+=1;
      // Clone/reorder Stage5 to prove equality is semantic, not object custody.
      snapshot.stage5Systems=Object.fromEntries(Object.entries(snapshot.stage5Systems).reverse());
      s31ReceiveOrdinary(f,snapshot);assert.equal(p.scope.ordinaryApplications.at(-1).full,false);
      assert.equal(p.scope.npcShopServiceRef.current,service);assert.equal(p.readNpcBuyCurrent().blocked,false);
      p.scope.worldRef.current={...p.scope.worldRef.current,groundDrops:[{objectId:"999",name:"stale quantity",quantity:99,x:0,y:0}],
        stage5Systems:{...p.scope.worldRef.current.stage5Systems,mail:[{gold:999}]}};
      assert.equal(p.npcPurchaseFullEconomyCurrent(p.scope.worldRef.current),false,"all modeled economics participate in display readiness");
      s31ReceiveOrdinary(f,snapshot);assert.equal(p.scope.ordinaryApplications.at(-1).full,true);
      assert.deepEqual(p.scope.worldRef.current.groundDrops,snapshot.groundDrops);assert.deepEqual(p.scope.worldRef.current.stage5Systems.mail,snapshot.stage5Systems.mail);
      snapshot.playerHp--;snapshot.playerExperience++;
      s31ReceiveOrdinary(f,snapshot);assert.equal(p.scope.ordinaryApplications.at(-1).full,true);
      assert.equal(p.scope.npcShopServiceRef.current,service);assert.equal(p.scope.npcPurchaseEconomicSourceRef.current,paired);
      const applied=f.abi.ops("applied").length;
      for(let read=0;read<2;read++)assert.equal(p.readNpcBuyCurrent().blocked,false);
      const id=service.buyItems[0].id;p.scope.npcBuySelectedRef.current=id;assert(p.dispatchDurableNpcPurchase(id,1));
      f.quote(snapshot);assert.equal(p.scope.npcShopServiceRef.current,service);
      const entered=f.ownerWires().at(-1);assert.equal(entered.action.kind,"purchase");assert.equal(f.abi.ops("applied").length,applied+1);
      const next=copy(snapshot);next.inventoryItems.push(s31WorldItem(11001+turn,1+turn));
      next.npcGoldTradeCapacity.freshCompatibleUniqueIds.push(11001+turn);next.nativeNpcShop=s31Shop(78+turn,profile,2-turn);
      const authority={...s30Authority,serverRevision:String(turn+1)};
      const receipt={producerScope:s30Scope,entry:{operation:entered.action.operation,serverRevision:String(turn+1),outcome:{committed:{
        request:entered.action.operation.intent.request,currency:entered.action.operation.intent.currency,source:entered.action.operation.intent.source,
        charged:1,admittedCount:1,incomingUniqueId:String(11001+turn)}}}};
      f.abi.state.mutate=(input,result)=>{if(input.op==="applied")f.abi.state.pending=null;return result;};
      f.client.receive(f.raw({kind:"purchase",receipt,replayed:false},next,authority,entered.requestId));snapshot=next;
    }
    assert.equal(f.abi.ops("reserve").length,2);assert.equal(f.abi.ops("enter").length,2);
    assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,2);assert.equal(f.abi.ops("begin").length,1);
  }
  assert(actualPageSource.includes("applyGatewayWorldSnapshot(snapshot, connectionGeneration, fullEconomy, qualifiedDisplay)"));
  assert(actualPageSource.includes("fullEconomy = fullEconomy || qualifiedDisplay;"),"equal-source full fallback replaces all packet economic overlays");
  // Execute the exact Page fallback assignment and reducers with deliberately
  // stale packet values. This is a reducer fixture, not a running world/UI.
  const nodes={};
  (function visit(node){
    if(ts.isExpressionStatement(node)&&node.getText(actualPageAst)==="fullEconomy = fullEconomy || qualifiedDisplay;")nodes.qualify=node.getText(actualPageAst);
    if(ts.isVariableDeclaration(node)&&["mergedEntitiesForWorld","mergedGroundDropsForWorld"].includes(node.name.getText(actualPageAst)))
      nodes[node.name.getText(actualPageAst)]=node.initializer.getText(actualPageAst);
    if(ts.isPropertyAssignment(node)&&node.name.getText(actualPageAst)==="stage5Systems"&&node.initializer.getText(actualPageAst).startsWith("fullEconomy ?"))
      nodes.stage=node.initializer.getText(actualPageAst);
    ts.forEachChild(node,visit);
  })(actualPageAst);
  assert.equal(Object.keys(nodes).length,4);
  const js=ts.transpileModule('return function(fullEconomy,qualifiedDisplay,snapshot,current,entities,groundDrops){'
    +'const packetRuntimeRefresh=true,currentTime=0,mergedEntities=entities;'+nodes.qualify
    +'const mergedEntitiesForWorld='+nodes.mergedEntitiesForWorld+';const mergedGroundDropsForWorld='+nodes.mergedGroundDropsForWorld+';'
    +'return {entities:mergedEntitiesForWorld,groundDrops:mergedGroundDropsForWorld,stage5Systems:'+nodes.stage+'};}',
    {compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
  const forbid=()=>assert.fail("qualified fallback used stale packet economic overlay");
  const fallback=new Function("mergePacketFirstSnapshotEntities","mergePacketFirstSnapshotGroundDrops","mailRawRef","socialFriendsRef",js)(
    forbid,forbid,{current:null},{current:null});
  const source={stage5Systems:{mail:[{gold:3}],mentor:{menteeExp:4}}},entities=[{quantity:2}],drops=[{quantity:5}];
  const result=fallback(false,true,source,{entities:[{quantity:99}],groundDrops:[{quantity:99}],stage5Systems:{mail:[{gold:99}]}},entities,drops);
  assert.equal(result.entities,entities);assert.equal(result.groundDrops,drops);assert.equal(result.stage5Systems,source.stage5Systems);
});

test("Source31 actual Page catalog-only periodic changes switch parent and retire gestures before or after Quote Enter",()=>{
  for(const edge of ["before","quote","enter"]){
    const f=s31PageOwner(),p=f.page,service=p.scope.npcShopServiceRef.current,applied=f.abi.ops("applied").length;
    const next=copy(f.snapshot);next.nativeNpcShop=s31Shop(78,"BUYUSED",2,51);
    next.entities.push({objectId:51,name:"Current second NPC",x:12,y:10,direction:"Down",dead:false});
    p.scope.npcBuySelectedRef.current=77;
    p.scope.beforeOrdinaryApply=(_snapshot,_full,qualified)=>{if(qualified){
      assert.equal(p.scope.npcShopServiceRef.current,null,"changed parent/catalog retired before world projection");
      assert.equal(p.dispatchDurableNpcPurchase(77,1),false);}};
    if(edge!=="before")assert(p.dispatchDurableNpcPurchase(77,1));
    if(edge==="enter")f.abi.state.mutate=(input,result)=>{if(input.op==="enter")s31ReceiveOrdinary(f,next);return result;};
    else s31ReceiveOrdinary(f,next);
    if(edge!=="before")f.quote();
    assert.notEqual(p.scope.npcShopServiceRef.current,service);assert.equal(p.scope.npcShopServiceRef.current.npcName,"Current second NPC");
    assert.equal(p.scope.npcShopServiceRef.current.buyItems[0].id,78);assert.equal(p.scope.npcBuySelectedRef.current,null);
    assert.equal(p.scope.ordinaryApplications.at(-1).full,true);assert.equal(f.abi.ops("applied").length,applied);
    assert.equal(f.abi.ops("begin").length,1);assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,0);
    if(edge==="before"){assert.equal(p.dispatchDurableNpcPurchase(77,1),false);p.scope.npcBuySelectedRef.current=78;
      assert(p.dispatchDurableNpcPurchase(78,1));f.quote(next);assert.equal(f.ownerWires().at(-1).action.kind,"purchase");}
    if(edge==="quote")assert.equal(f.abi.ops("reserve").length,0);
    if(edge==="enter"){assert.equal(f.abi.state.pending.phase,"unknown");assert.deepEqual(f.abi.ops("unknown")[0].operation,f.abi.state.pending.operation);}
  }
});

test("Source31 ordinary complete changes never settle retained operation or create Begin Query and missing catalog withdraws",()=>{
  const f=s31PageOwner(),p=f.page;p.scope.npcBuySelectedRef.current=77;assert(p.dispatchDurableNpcPurchase(77,1));f.quote();
  const operation=copy(f.abi.state.pending.operation),paired=p.scope.npcPurchaseEconomicSourceRef.current,applied=f.abi.ops("applied").length;
  const next=copy(f.snapshot);next.gold--;next.stage5Systems.mentor.menteeExp++;
  next.inventoryItems.push(s31WorldItem(12001,1));next.npcGoldTradeCapacity.freshCompatibleUniqueIds.push(12001);
  s31ReceiveOrdinary(f,next);assert.deepEqual(f.abi.state.pending.operation,operation);assert.equal(f.abi.state.pending.phase,"entered");
  assert.equal(p.scope.npcPurchaseEconomicSourceRef.current,paired);assert.equal(f.abi.ops("applied").length,applied);
  assert.equal(f.abi.ops("begin").length,1);assert.equal(f.ownerWires().filter(w=>w.action.kind==="query").length,0);
  assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,1);
  next.nativeNpcShop=null;s31ReceiveOrdinary(f,next);assert.equal(p.scope.npcShopServiceRef.current,null);
  assert.deepEqual(f.abi.state.pending.operation,operation);assert.equal(f.abi.ops("applied").length,applied);
  const display=p.scope.npcPurchaseDisplaySourceRef.current;
  p.applyNpcPurchaseOrdinarySnapshot(next,s31OrdinaryRaw(next),0,p.socket);assert.equal(p.scope.npcPurchaseDisplaySourceRef.current,display);
  p.applyNpcPurchaseOrdinarySnapshot(next,s31OrdinaryRaw(next),1,{});assert.equal(p.scope.npcPurchaseDisplaySourceRef.current,display);
  p.scope.npcPurchaseApplyingEconomyRef.current=true;
  p.applyNpcPurchaseOrdinarySnapshot(next,s31OrdinaryRaw(next),1,p.socket);assert.equal(p.scope.npcPurchaseDisplaySourceRef.current,display);
});

test("Source31 raw marker scans every escaped duplicate top-level type and strict owner never reaches ordinary decoder",()=>{
  const owner='{ "ty\\u0070e":"npcPurchaseOwner","type":"worldSnapshot","payload":{} }';
  const reverse='{"type":"worldSnapshot","payload":{},"\\u0074ype":"npcPurchaseOwner"}';
  const deep='{"ignored":'+"[".repeat(80)+"0"+"]".repeat(80)+',"type":"npcPurchaseOwner","type":"worldSnapshot"}';
  for(const raw of [owner,reverse,deep,'{"type":"npcPurchaseOwner","broken":']){
    assert(actualDurableNpc.isNpcPurchaseOwnerFrame(raw));const f=s31PageOwner(),applied=f.abi.ops("applied").length;
    const ordinary=f.page.scope.ordinaryApplications.length;f.page.receiveNpcPurchaseGatewayFrame({data:raw},1,f.page.socket);
    assert.equal(f.page.scope.ordinaryApplications.length,ordinary);assert.equal(f.abi.ops("applied").length,applied);
  }
  for(const raw of ['{"type":"packet","payload":{"type":"npcPurchaseOwner"}}','{"type":"packet","text":"npcPurchaseOwner"}'])
    assert.equal(actualDurableNpc.isNpcPurchaseOwnerFrame(raw),false);
  for(const atEnd of [false,true]){
    const f=s31PageOwner(),p=f.page;p.scope.npcBuySelectedRef.current=77;assert(p.dispatchDurableNpcPurchase(77,1));f.quote();
    const operation=copy(f.abi.state.pending.operation),ordinary=p.scope.ordinaryApplications.length;
    const padding='"padding":"'+"x".repeat(16*1024*1024)+'"';
    const raw=atEnd?'{"type":"worldSnapshot",'+padding+',"type":"npcPurchaseOwner"}'
      :'{"type":"npcPurchaseOwner",'+padding+',"type":"worldSnapshot"}';
    assert.throws(()=>p.receiveNpcPurchaseGatewayFrame({data:raw},1,p.socket),/size/);
    assert.equal(p.scope.ordinaryApplications.length,ordinary);assert.equal(p.scope.npcPurchaseDisplaySourceRef.current,null);
    assert.equal(p.scope.npcShopServiceRef.current,null);assert.deepEqual(f.abi.state.pending.operation,operation);
    assert.equal(f.ownerWires().filter(w=>w.action.kind==="purchase").length,1);
  }
});

test("Source31 ordinary unsafe or duplicate world fails closed without rounded fallback or paired source replacement",()=>{
  for(const edge of ["unsafe","duplicate","deep"]){
    const f=s31PageOwner(),p=f.page,paired=p.scope.npcPurchaseEconomicSourceRef.current,ordinary=p.scope.ordinaryApplications.length;
    let raw=s31OrdinaryRaw(f.snapshot);
    if(edge==="unsafe")raw=raw.replace('"tick":0','"tick":9007199254740993');
    if(edge==="duplicate")raw=raw.replace('"type":"worldSnapshot"','"type":"worldSnapshot","ty\\u0070e":"worldSnapshot"');
    if(edge==="deep")raw=raw.replace('"payload":{','"payload":{"unknown":'+"[".repeat(70)+"0"+"]".repeat(70)+',');
    assert.throws(()=>p.receiveNpcPurchaseGatewayFrame({data:raw},1,p.socket),undefined,edge);
    assert.equal(p.scope.ordinaryApplications.length,ordinary,edge);assert.equal(p.scope.npcPurchaseDisplaySourceRef.current,null,edge);
    assert.equal(p.scope.npcShopServiceRef.current,null,edge);assert.equal(p.scope.npcPurchaseEconomicSourceRef.current,paired,edge);
    assert.equal(f.abi.ops("begin").length,1,edge);assert.equal(p.dispatchDurableNpcPurchase(77,1),false,edge);
    assert.equal(p.scope.npcPurchaseUnavailableRef.current,true,edge);
    // Only an actual service opening can request a fresh paired Begin after
    // withdrawal; equivalent periodic display must not regain Core authority.
    const shop=f.snapshot.nativeNpcShop;
    p.applyNpcShopCatalogPacket("NPCGoods",{list:shop.displayGoods,rate:shop.rate,panelType:shop.panelType,hideAddedStats:shop.hideAddedStats});
    assert.equal(f.abi.ops("begin").length,2,edge);f.begin(f.snapshot);
    assert.equal(p.scope.npcPurchaseUnavailableRef.current,false,edge);assert.equal(p.readNpcBuyCurrent().blocked,false,edge);
  }
});

test("Source31 exact integer consumers pass bounded strict no-emit type checking",()=>{
  const files=["../lib/npc-purchase-client.ts","../lib/npc-purchase-receipt.ts","../lib/world-model/item-identity.ts","../lib/bevy-bag-model.ts",
    "../lib/extended-server-packets.ts","../lib/storage-rental-confirmation.ts","../lib/hero-player-ui.ts","../lib/bevy-storage-model.ts"].map(relative=>fileURLToPath(new URL(relative,import.meta.url)));
  const program=ts.createProgram(files,{noEmit:true,incremental:false,strict:true,target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,
    moduleResolution:ts.ModuleResolutionKind.Bundler,lib:["lib.es2022.d.ts","lib.dom.d.ts"],types:[]});
  const diagnostics=ts.getPreEmitDiagnostics(program);assert.equal(diagnostics.length,0,ts.formatDiagnostics(diagnostics,{getCanonicalFileName:file=>file,getCurrentDirectory:()=>process.cwd(),getNewLine:()=>"\n"}));
});

test("Source31 storage DOM decodes canonical wide i64 kind and exact epoch ticks without rounding",()=>{
  const source=readFileSync(new URL("../app/components/original-client-inventory-utils.ts",import.meta.url),"utf8");
  const ast=ts.createSourceFile("actual-storage-date-utils.ts",source,ts.ScriptTarget.Latest,true);
  const names=["formatBinaryDateTimeLabel","dateFromBinaryDateTime"],declarations=ast.statements
    .filter(node=>ts.isFunctionDeclaration(node)&&names.includes(node.name?.text)).map(node=>node.getText(ast));
  assert.equal(declarations.length,2);
  const output=ts.transpileModule(declarations.join("\n"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  const module={exports:{}};new Function("exports",output)(module.exports);
  const format=module.exports.formatBinaryDateTimeLabel;
  const expected=local=>new Intl.DateTimeFormat("en-GB",{year:"numeric",month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit",timeZone:local?undefined:"UTC"}).format(new Date(0));
  // One tick after the Unix epoch stays exact; kind bits are never a signed
  // Number. Exercise the actual formatter that the inventory window calls.
  for(const [raw,local] of [["621355968000000001",false],[(621355968000000001n|0x4000000000000000n).toString(),false],
    [BigInt.asIntN(64,621355968000000001n|0x8000000000000000n).toString(),true]]){
    assert.equal(format("en-GB",raw,"Expires: {0}"),"Expires: "+expected(local));
    assert.equal(typeof raw,"string");
  }
  for(const value of [0,Number("621355968000000001"),"01","+1","1e3","-0","9223372036854775807","9223372036854775808","-9223372036854775809"])
    assert.equal(format("en-GB",value,"Expires: {0}"),null,String(value));
  assert(format("en-GB",Number.MAX_SAFE_INTEGER,"Expires: {0}"));
  const inventory=readFileSync(new URL("../app/components/original-client-inventory-window.tsx",import.meta.url),"utf8");
  assert.match(inventory,/formatBinaryDateTimeLabel\([\s\S]{0,100}world\.storagePasswordLastSetBinaryDatetime/);
  assert.match(inventory,/formatBinaryDateTimeLabel\([\s\S]{0,100}world\.expandedStorageExpiryTimeBinaryDatetime/);
  const types=readFileSync(new URL("../app/components/original-client-types.ts",import.meta.url),"utf8");
  for(const name of ["storagePasswordLastSetBinaryDatetime","expandedStorageExpiryTimeBinaryDatetime"])
    assert(types.includes(name+': WorldState["'+name+'"]'));
  const barrel=readFileSync(new URL("../lib/world-model/index.ts",import.meta.url),"utf8");
  assert.match(barrel,/export \{[^}]*\bexactItemUniqueId\b[^}]*\} from "\.\/item-identity"/);
});

test("Source35 exact storage ABI preserves signed i64 text and rejects unsafe numeric dates",()=>{
  const storage=loadExpiryPipelineModule("../lib/bevy-storage-model.ts",{"./bevy-bag-model":expiryBag,"./npc-purchase-client":actualDurableNpc});
  const world={inventoryCapacity:46,maxBagSlots:40,gold:100,inventoryItems:[],beltItems:[],equipmentItems:[],storageItems:[],storageSize:80,
    hasStoragePassword:false,storageSessionUnlocked:true,hasExpandedStorage:false,expandedStorageExpiryTimeBinaryDatetime:1000};
  for(const expiry of [0,1000,Number.MIN_SAFE_INTEGER,Number.MAX_SAFE_INTEGER]){
    const p=storage.projectBevyStorageModel({...world,expandedStorageExpiryTimeBinaryDatetime:expiry});
    assert.equal(p.ok,true);assert.equal(p.storage.expiry,expiry);
  }
  for(const expiry of ["0","1000","621355968000000001","621355968000000002","-9223372036854775808","9223372036854775807"]){
    const input={...world,expandedStorageExpiryTimeBinaryDatetime:expiry};
    const p=storage.projectBevyStorageModel(input);
    assert.equal(p.ok,true,expiry);assert.equal(p.storage.expiry,expiry);
    assert.equal(input.expandedStorageExpiryTimeBinaryDatetime,expiry);
  }
  for(const expiry of ["+1","-0","01","-01"," 1","1 ","1e3","1.0","9223372036854775808","-9223372036854775809",Number("621355968000000001"),Number.MIN_SAFE_INTEGER-1,NaN,Infinity,1.5]){
    const input={...world,expandedStorageExpiryTimeBinaryDatetime:expiry};
    const p=storage.projectBevyStorageModel(input);
    assert.equal(p.ok,false,String(expiry));assert.equal(p.error.field,"storage");
    assert.equal(input.expandedStorageExpiryTimeBinaryDatetime,expiry);
  }
});
