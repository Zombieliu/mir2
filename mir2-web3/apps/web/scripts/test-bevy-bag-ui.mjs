import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

function load(path, requireLocal = () => { throw new Error("unexpected import"); }) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } });
  const module = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, requireLocal);
  return module.exports;
}
const identity = load("../lib/world-model/item-identity.ts");
const bagBelt = load("../lib/bag-belt-gesture.ts");
const bag = load("../lib/bevy-bag-ui.ts", path => path === "./bag-belt-gesture" ? bagBelt : identity);
const model = () => ({ capacity: 46, gold: 200, items: [{ container: 0, slot: 0, uniqueId: 0 }] });
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false };

function fixture(overrides = {}, options = {}) {
  let now = 100;
  let input = { connectionGeneration: 3, sessionGeneration: 5, ownerRevision: 0,
    eligible: true, bagOpen: true, page: "bag1", presentation, model: model(), player: { level: 1 }, blockedUniqueIds: [] };
  let status = null;
  let sink = null;
  let state = null;
  const snapshots = [], owners = [], intents = [], edges = [];
  const runtime = {
    getMir2BagUiAbiVersion: () => 1,
    setMir2BagUiSnapshot(json) { snapshots.push(JSON.parse(json)); return true; },
    getMir2BagUiStatus: () => JSON.stringify(status),
    setMir2BagUiIntentSink(fn) { sink = fn; },
    clearMir2BagUiIntentSink() { sink = null; },
    setMir2BagUiPointerEdge(json) { edges.push(JSON.parse(json)); return true; },
    ...overrides,
  };
  const host = new bag.BevyBagHost({ runtime, now: () => now, read: () => input,
    onOwner(owner, runGeneration, ownerRevision) { owners.push({ owner, runGeneration, ownerRevision }); input.ownerRevision = ownerRevision; },
    onState(next) { state = next; }, onIntent(intent) { intents.push(intent); return options.onIntent?.(intent) ?? { accepted: true }; },
  });
  function echo(extra = {}) {
    const s = snapshots.at(-1);
    status = { runGeneration: s.runGeneration, connectionGeneration: s.connectionGeneration, sessionGeneration: s.sessionGeneration,
      ownerRevision: s.ownerRevision, frame: (status?.frame ?? 0) + 1, ready: true, inputEnabled: s.inputEnabled,
      appliedRevision: s.revision, appliedModelRevision: s.modelRevision, appliedPresentationRevision: s.presentationRevision,
      inputRegions: [{ left: 700, top: 40, width: 316, height: 236 }], error: null, ...extra };
  }
  function activate() { host.tick(); echo(); host.tick(); echo(); host.tick(); assert.equal(state.active, true); }
  return { host, runtime, snapshots, owners, intents, edges, echo, activate,
    get input() { return input; }, set input(next) { input = next; }, get state() { return state; },
    time(ms) { now += ms; },
    intent(extra = {}) { const s = snapshots.at(-1); return { runGeneration: s.runGeneration,
      connectionGeneration: s.connectionGeneration, sessionGeneration: s.sessionGeneration, ownerRevision: s.ownerRevision,
      modelRevision: s.modelRevision, presentationRevision: s.presentationRevision, intentSequence: 1,
      type: "useItem", source: { container: 0, slot: 0, uniqueId: 0 }, ...extra }; },
    emit(intent) { return JSON.parse(sink(JSON.stringify(intent))); }, get sink() { return sink; },
  };
}

test("ABI/model validation keeps UID0, exact capacities, duplicate and source authority boundaries", () => {
  assert.equal(bag.validBagModel(model()), true);
  for (const capacity of [0, 40, 47, 55, 87]) assert.equal(bag.validBagModel({ ...model(), capacity }), false);
  assert.equal(bag.validBagModel({ ...model(), items: [...model().items, { container: 0, slot: 1, uniqueId: 0 }] }), false);
  assert.equal(bag.validBagModel({ ...model(), items: [...model().items, { container: 0, slot: 0, uniqueId: null }] }), false);
  assert.equal(bag.validBagModel({ ...model(), items: [{ container: 0, slot: 40, uniqueId: 2 }] }), false);
  const f = fixture(); f.activate();
  assert.equal(bag.parseBagIntent(JSON.stringify(f.intent())).source.uniqueId, 0);
  for (const source of [{ container: 3, slot: 0, uniqueId: 0 }, { container: 0, slot: 0 }, { container: 0, slot: 80, uniqueId: 0 }]) {
    assert.equal(bag.parseBagIntent(JSON.stringify(f.intent({ source }))), null);
  }
});

test("two-phase ownership waits through several slow frames for exact enabled echo", () => {
  const f = fixture(); f.host.tick(); f.echo(); f.host.tick();
  const epoch = f.owners.at(-1);
  assert.equal(epoch.owner, "transition");
  assert.equal(f.state.active, false);
  for (let i = 0; i < 5; i++) { f.time(100); f.host.tick(); assert.deepEqual(f.owners.at(-1), epoch); }
  f.echo({ appliedPresentationRevision: 0 }); f.host.tick(); assert.equal(f.state.active, false);
  f.echo(); f.host.tick(); assert.equal(f.state.active, true);
  assert.equal(f.owners.at(-1).ownerRevision, epoch.ownerRevision);
  assert.equal(f.owners.at(-1).owner, "bevy");
});

test("old owner/model/sequence intents are refused without upgrading their token", () => {
  const f = fixture(); f.activate();
  const old = f.intent();
  assert.equal(f.emit(old).accepted, true);
  assert.equal(f.emit(old).accepted, false);
  f.input.model = { ...model(), gold: 201 };
  assert.equal(f.emit(f.intent({ intentSequence: 2 })).accepted, false, "live model changes reject before poll");
  f.host.tick(); f.echo(); f.host.tick();
  assert.equal(f.state.active, true, "ordinary model refresh does not flicker owners");
  assert.equal(f.emit(f.intent({ intentSequence: 2 })).accepted, true);
  f.host.yieldToReact();
  assert.equal(f.emit({ ...old, intentSequence: 3 }).accepted, false);
  assert.equal(f.snapshots.at(-1).inputEnabled, false);
  assert.equal(f.intents.length, 2);
});

test("an advancing renderer with an unapplied handoff cannot strand React input in transition", () => {
  for (const badEcho of [{ ready: false }, { appliedModelRevision: 0 }, { inputEnabled: false }]) {
    const f = fixture(); f.host.tick(); f.echo(); f.host.tick();
    assert.equal(f.owners.at(-1).owner, "transition");
    for (let index = 0; index < 5; index++) { f.time(450); f.echo(badEcho); f.host.tick(); }
    assert.equal(f.owners.at(-1).owner, "react");
    assert.equal(f.state.active, false);
    assert.equal(f.snapshots.at(-1).inputEnabled, false);
  }
});

test("frame watchdog checks synchronously at sink/send, then a new frame can restore ownership", () => {
  const f = fixture(); f.activate(); const old = f.intent();
  f.time(2001);
  assert.equal(f.emit(old).accepted, false);
  assert.equal(f.state.active, false);
  f.host.tick(); f.echo(); f.host.tick(); f.echo(); f.host.tick();
  assert.equal(f.state.active, true);
  assert.equal(f.emit(old).accepted, false);
  assert.equal(f.emit(f.intent()).accepted, true);
});

test("missing ABI, invalid layout, touch, focus/visibility eligibility and invalid models keep React", () => {
  const missing = fixture({ getMir2BagUiAbiVersion: () => 0 }); missing.host.tick();
  assert.equal(missing.state.active, false); assert.equal(missing.snapshots.length, 0);
  for (const patch of [{ presentation: null }, { presentation: { ...presentation, touch: true } },
    { eligible: false }, { model: null }]) {
    const f = fixture(); f.activate(); Object.assign(f.input, patch); f.host.tick();
    assert.equal(f.state.active, false);
    assert.equal(f.snapshots.at(-1).inputEnabled, false);
    f.host.tick(); assert.equal(f.state.active, false);
  }
});

test("wide touch geometry alone enters ABI1 while portrait, letterbox and short stages stay compatible", () => {
  const wide640 = { logicalWidth: 1368, logicalHeight: 768, stageCssScale: 640 / 1368, touch: true };
  const wide844 = { logicalWidth: 1664, logicalHeight: 768, stageCssScale: 844 / 1664, touch: true };
  for (const shape of [wide640, wide844]) {
    assert.equal(bag.eligibleBagPresentation(shape), true);
    const f = fixture(); f.input.presentation = shape; f.activate();
    assert.equal(f.snapshots.at(-1).presentation.touch, true);
    assert.equal(f.emit(f.intent()).accepted, true);
  }
  for (const shape of [
    { ...wide640, stageCssScale: 599 / 1368 },
    { ...wide640, stageCssScale: 319 / 768 },
    { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 640 / 1024, touch: true },
    { logicalWidth: 768, logicalHeight: 1368, stageCssScale: 640 / 768, touch: true },
    { ...wide640, stageCssScale: Number.NaN },
    { ...wide640, logicalWidth: 1368.5 },
  ]) {
    assert.equal(bag.eligibleBagPresentation(shape), false);
    const f = fixture(); f.input.presentation = shape; f.host.tick();
    assert.equal(f.state.active, false);
  }
});

test("session, resize and restart invalidate prior callbacks while pending flags survive handoff", () => {
  const f = fixture(); f.activate(); const old = f.intent();
  f.input.blockedUniqueIds = [0];
  assert.equal(f.host.allows({ ...old, owner: "bevy" }), true, "own reservation does not masquerade as model replacement");
  f.host.yieldToReact();
  assert.deepEqual(f.input.blockedUniqueIds, [0]);
  f.input.presentation = { ...presentation, stageCssScale: 0.8 };
  f.host.tick(); f.echo(); f.host.tick(); f.echo(); f.host.tick();
  assert.equal(f.emit(old).accepted, false);
  f.input.sessionGeneration++; assert.equal(f.emit(f.intent({ intentSequence: 5 })).accepted, false);
  const staleSink = f.sink; f.host.stop();
  assert.equal(JSON.parse(staleSink(JSON.stringify(old))).accepted, false);
  const next = fixture(); next.activate();
  assert.notEqual(next.intent().runGeneration, old.runGeneration);
  assert.equal(next.emit(old).accepted, false);
});

test("pointer down origin survives crossing the bag, including same-frame pulse", () => {
  const f = fixture(); f.activate(); const context = f.host.pointerContext(); const router = new bag.BagPointerRouter();
  assert.equal(router.down(context, 1, 0, 720, 90).origin, "bag");
  assert.equal(router.edge("move", 1, 400, 450).origin, "bag");
  assert.equal(router.edge("up", 1, 400, 450).origin, "bag");
  assert.equal(router.down(context, 1, 0, 400, 450).origin, "world");
  assert.equal(router.edge("move", 1, 720, 90).origin, "world");
  assert.equal(router.edge("up", 1, 720, 90).origin, "world");
  const down = router.down(context, 1, 2, 400, 450), up = router.edge("up", 1, 400, 450);
  assert.equal(down.origin, "world"); assert.equal(up.origin, "world"); assert.ok(up.sequence > down.sequence);
});

test("Belt-capable production router sends only strict pointer fields and retains local drop geometry", () => {
  const wireFields = ["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision",
    "presentationRevision", "sequence", "pointerId", "phase", "origin", "x", "y", "button"].sort();
  const wire = [];
  const f = fixture({ setMir2BagUiPointerEdge(json) {
    const edge = JSON.parse(json);
    assert.deepEqual(Object.keys(edge).sort(), wireFields);
    wire.push(edge);
    return true;
  } });
  const geometry = { revision: 7, targets: Array.from({ length: 6 }, (_, slot) => ({
    slot, uniqueId: null, left: 50 + slot * 40, top: 650, width: 32, height: 32,
  })) };
  assert.equal(bagBelt.validBagBeltDropGeometry(geometry), true);
  f.input = { ...f.input, beltDropGeometry: geometry };
  for (let i = 0; i < 4; i++) {
    f.host.tick();
    f.echo({ supportsBagToBelt: true, appliedBeltGeometryRevision: geometry.revision });
  }
  assert.equal(f.state.active, true);
  const context = f.host.pointerContext(), router = new bag.BagPointerRouter();
  assert.ok(context);
  assert.equal(context.beltDropGeometry, geometry);
  const target = context.beltDropGeometry.targets[2];
  const x = target.left + target.width / 2, y = target.top + target.height / 2;
  const edges = [router.down(context, 1, 0, 720, 90),
    router.edge("move", 1, x, y), router.edge("up", 1, x, y)];
  for (const edge of edges) assert.equal(f.host.pointer(edge), true);
  assert.deepEqual(wire.map(edge => edge.phase), ["down", "move", "up"]);
  assert.ok(wire.every(edge => edge.origin === "bag" && edge.runGeneration === context.runGeneration
    && edge.connectionGeneration === context.connectionGeneration && edge.sessionGeneration === context.sessionGeneration
    && edge.ownerRevision === context.ownerRevision && edge.presentationRevision === context.presentationRevision));
  assert.ok(wire[0].sequence < wire[1].sequence && wire[1].sequence < wire[2].sequence);
  assert.deepEqual([wire[2].x, wire[2].y], [x, y]);
  assert.equal(router.held, null);
  for (const terminal of ["cancel", "blur"]) {
    assert.equal(f.host.pointer(router.down(context, 1, 0, 720, 90)), true);
    assert.equal(f.host.pointer(router.cancel(terminal)), true);
    assert.equal(f.host.pointer(router.edge("up", 1, x, y)), true);
  }
  assert.equal(context.beltDropGeometry, geometry, "wire filtering cannot retire the local Belt drop geometry");
  assert.equal(context.beltDropGeometry.targets[2], target);
});

test("blur or owner handoff permits a fresh reused pointerId even when the old up never arrives", () => {
  const f = fixture(); f.activate(); const router = new bag.BagPointerRouter();
  const old = router.down(f.host.pointerContext(), 1, 0, 720, 90);
  assert.equal(f.host.pointer(old), true);
  f.host.yieldToReact();
  assert.equal(f.host.pointer(router.cancel()), true);
  assert.equal(router.edge("move", 1, 400, 450), null, "old motion cannot open a new lease");
  f.host.tick(); f.echo(); f.host.tick(); f.echo(); f.host.tick();
  assert.equal(f.state.active, true);
  const fresh = router.down(f.host.pointerContext(), 1, 0, 720, 90);
  assert.ok(fresh);
  assert.notEqual(fresh.ownerRevision, old.ownerRevision);
  assert.equal(f.host.pointer(fresh), true);
  assert.equal(f.host.pointer(router.edge("up", 1, 720, 90)), true);
  assert.equal(f.edges.at(-1).phase, "up");
});

test("NPC server-attested alias gate preserves ordinary Bag and validates each listed carried root exactly once", () => {
  const mixed={capacity:46,gold:100,items:[{container:0,slot:0,uniqueId:0},{container:1,slot:0,uniqueId:0}],
    npcGoldTradeCapacity:{rosterValid:true,freshCompatibleUniqueIds:[]}};
  assert.equal(bag.validBagModel(mixed),false);assert.equal(bag.validNpcGoldBuyBagModel(mixed),true);
  assert.equal(bag.validNpcGoldBuyBagModel({...mixed,npcGoldTradeCapacity:null}),false);
  for(const evidence of [
    {rosterValid:true,freshCompatibleUniqueIds:[0]},{rosterValid:true,freshCompatibleUniqueIds:[10]},
    {rosterValid:true,freshCompatibleUniqueIds:[10,10]},{rosterValid:false,freshCompatibleUniqueIds:[10]},
    {rosterValid:true,freshCompatibleUniqueIds:[Number.MAX_SAFE_INTEGER+1]},
    {rosterValid:true},{rosterValid:true,freshCompatibleUniqueIds:[],extra:true}
  ])assert.equal(bag.validNpcGoldBuyBagModel({...mixed,npcGoldTradeCapacity:evidence}),false);
  const valid={...mixed,items:[{container:0,slot:0,uniqueId:10}],
    npcGoldTradeCapacity:{rosterValid:true,freshCompatibleUniqueIds:[10]}};
  assert(bag.validNpcGoldBuyBagModel(valid));
  assert.equal(bag.validNpcGoldBuyBagModel({...valid,items:[{container:2,slot:0,uniqueId:10}]}),false);
  assert.equal(bag.validNpcGoldBuyBagModel({...mixed,items:[{container:0,slot:0,uniqueId:0},{container:0,slot:1,uniqueId:0}]}),false);
});

function resourceFixture(options = {}) {
  const f = fixture({}, options);
  f.input.player = { hp: 18, maxHp: 18, mp: 4, maxMp: 8, level: 1, className: "Warrior", gender: "Male",
    name: "private-player-name", gold: 200, credit: 0 };
  f.input.model.items[0] = { ...f.input.model.items[0], key: "private-potion-key", name: "private-potion-name",
    quantity: 2, tooltipSource: { info: { itemType: 13, shape: 0 } } };
  return f;
}

test("real Host accepts a single already-painted Native intent after positive HP/MP changes before the next poll", () => {
  for (const patch of [{ hp: 17 }, { hp: 1 }, { mp: 3 }, { mp: 7 }, { hp: 12, mp: 2 }]) {
    const f = resourceFixture(); f.activate();
    const painted = f.intent(), owner = f.owners.at(-1), prior = f.snapshots.at(-1);
    f.input.player = { ...f.input.player, ...patch };
    assert.equal(f.emit(painted).accepted, true, JSON.stringify(patch));
    assert.equal(f.host.allows({ ...painted, owner: "bevy" }), true, "the final Page send gate retains the same model proof");
    assert.equal(f.intents.length, 1); assert.equal(f.emit(painted).accepted, false, "the accepted physical intent is never replayed");
    f.host.tick();
    const display = f.snapshots.at(-1);
    assert.equal(display.modelRevision, prior.modelRevision, "cosmetic resources do not replace the input model");
    assert.ok(display.revision > prior.revision, "full resources still publish a new display snapshot");
    assert.deepEqual(display.player, f.input.player); assert.equal(f.state.active, true); assert.deepEqual(f.owners.at(-1), owner);
    f.echo(); f.host.tick(); assert.equal(f.intents.length, 1, "polling and echo never retry an item action");
  }
});

test("real Host retains the current model gate while a positive-resource display snapshot is waiting for Native echo", () => {
  for (const patch of [{ hp: 17 }, { mp: 3 }, { hp: 1, mp: 7 }]) {
    const f = resourceFixture(); f.activate();
    const painted = f.intent(), before = f.snapshots.at(-1), owner = f.owners.at(-1);
    f.input.player = { ...f.input.player, ...patch }; f.host.tick();
    const display = f.snapshots.at(-1);
    assert.deepEqual(display.player, f.input.player, "the new HP/MP values are sent to the painter without an input-model bump");
    assert.ok(display.revision > before.revision);
    assert.equal(display.modelRevision, painted.modelRevision);
    assert.equal(f.emit(painted).accepted, true, "old applied Native model is still the exact current input model");
    assert.equal(f.intents.length, 1); assert.equal(f.emit(painted).accepted, false);
    f.echo(); f.host.tick(); assert.equal(f.intents.length, 1); assert.deepEqual(f.owners.at(-1), owner);
  }
});

test("life/mana known-positive boundaries and all material player, inventory, identity and layout changes reject old Native intents", () => {
  const cases = [
    ["HP zero", f => { f.input.player.hp = 0; }], ["HP unknown", f => { delete f.input.player.hp; }],
    ["MP zero", f => { f.input.player.mp = 0; }], ["MP unknown", f => { delete f.input.player.mp; }],
    ["HP unknown to known zero", f => { f.input.player.hp = 0; }, { hp: undefined }],
    ["HP dead to alive", f => { f.input.player.hp = 1; }, { hp: 0 }],
    ["HP unknown to alive", f => { f.input.player.hp = 1; }, { hp: undefined }],
    ["MP unknown to known zero", f => { f.input.player.mp = 0; }, { mp: undefined }],
    ["MP zero to positive", f => { f.input.player.mp = 1; }, { mp: 0 }],
    ["MP unknown to positive", f => { f.input.player.mp = 1; }, { mp: undefined }],
    ["HP invalid fraction", f => { f.input.player.hp = 1.5; }], ["MP invalid infinity", f => { f.input.player.mp = Infinity; }],
    ["HP null", f => { f.input.player.hp = null; }], ["HP outside signed resource range", f => { f.input.player.hp = 0x8000_0000; }],
    ["max HP", f => { f.input.player.maxHp++; }], ["max MP", f => { f.input.player.maxMp++; }],
    ["level", f => { f.input.player.level++; }], ["class", f => { f.input.player.className = "Wizard"; }],
    ["gender", f => { f.input.player.gender = "Female"; }], ["name", f => { f.input.player.name = "replacement-player"; }],
    ["player gold", f => { f.input.player.gold++; }], ["credit", f => { f.input.player.credit++; }],
    ["inventory gold", f => { f.input.model.gold++; }], ["item quantity", f => { f.input.model.items[0].quantity--; }],
    ["item UID", f => { f.input.model.items[0].uniqueId = 10; }], ["item slot", f => { f.input.model.items[0].slot = 1; }],
    ["item metadata", f => { f.input.model.items[0].tooltipSource.info.shape = 1; }],
    ["owner", f => { f.input.ownerRevision++; }], ["connection", f => { f.input.connectionGeneration++; }],
    ["session", f => { f.input.sessionGeneration++; }], ["presentation", f => { f.input.presentation = { ...presentation, stageCssScale: 0.8 }; }],
    ["page", f => { f.input.page = "bag2"; }], ["hidden", f => { f.input.bagOpen = false; }],
    ["host eligibility", f => { f.input.eligible = false; }]
  ];
  for (const [label, change, initial] of cases) for (const poll of [false, true]) {
    const f = resourceFixture(); Object.assign(f.input.player, initial); f.activate();
    const painted = f.intent(); change(f); if (poll) f.host.tick();
    assert.equal(f.emit(painted).accepted, false, `${label}/${poll ? "pending snapshot" : "before poll"}`);
    assert.equal(f.intents.length, 0, label);
  }
});

test("intent verdict diagnostics are bounded, private-data-free, read-only and never invoke another runtime getter", () => {
  let outcome = "accepted";
  const f = resourceFixture({ onIntent: () => {
    if (outcome === "throw") throw Error("private callback details");
    return outcome === "accepted" ? { accepted: true } : { accepted: false, error: "private rejection details" };
  } });
  f.activate();
  const native = f.intent(), getStatus = f.runtime.getMir2BagUiStatus;
  let statusReads = 0;
  f.runtime.getMir2BagUiStatus = () => { statusReads++; return getStatus(); };
  const emit = sequence => {
    const before = statusReads, result = f.emit({ ...native, intentSequence: sequence });
    assert.equal(statusReads - before, 2, "only the existing healthyStatus and layout gate may read Native");
    return result;
  };
  assert.equal(emit(1).accepted, true);
  outcome = "rejected"; assert.equal(emit(2).accepted, false);
  outcome = "throw"; assert.equal(emit(3).accepted, false);
  outcome = "accepted"; assert.equal(emit(3).accepted, false, "Page failure still consumes that intent sequence");
  const beforeRead = statusReads, evidence = f.host.diagnostic().intentVerdicts;
  assert.equal(statusReads, beforeRead); assert.equal(evidence.length, 4);
  assert.deepEqual(evidence.map(row => row.verdict), ["accepted", "page-rejected", "page-rejected", "host-rejected"]);
  const fields = ["verdict", "type", "hostModelRevision", "nativeModelRevision", "hostPresentationRevision", "nativePresentationRevision", "intentSequence"].sort();
  for (const row of evidence) {
    assert.deepEqual(Object.keys(row).sort(), fields); assert.equal(row.type, "useItem");
    assert.equal(row.hostModelRevision, native.modelRevision); assert.equal(row.nativeModelRevision, native.modelRevision);
    assert.equal(row.hostPresentationRevision, native.presentationRevision); assert.equal(row.nativePresentationRevision, native.presentationRevision);
  }
  evidence[0].verdict = "tampered"; evidence.length = 0;
  assert.equal(f.host.diagnostic().intentVerdicts[0].verdict, "accepted", "callers cannot mutate private evidence");
  for (let i = 0; i < 10; i++) assert.equal(emit(3).accepted, false);
  assert.equal(f.host.diagnostic().intentVerdicts.length, 8);
  const json = JSON.stringify(f.host.diagnostic().intentVerdicts);
  for (const privateText of ["private-player-name", "private-potion", "private callback", "private rejection", "uniqueId", "source", "raw"])
    assert.equal(json.includes(privateText), false, privateText);
  const frozen = statusReads; f.host.diagnostic(); f.host.diagnostic(); assert.equal(statusReads, frozen);
  assert.equal(f.intents.length, 3, "the Page sink runs once per accepted Host sequence and is never retried");
});

test("positive resource updates preserve Page reservation rejection without replay after its block clears", () => {
  const f = resourceFixture({ onIntent: intent => ({ accepted: !f.input.blockedUniqueIds.includes(intent.source.uniqueId) }) });
  f.activate(); const painted = f.intent();
  f.input.player.hp = 17; f.input.blockedUniqueIds = [0];
  assert.equal(f.emit(painted).accepted, false); assert.equal(f.intents.length, 1, "the unchanged current model reaches the existing Page gate once");
  assert.equal(f.host.diagnostic().intentVerdicts.at(-1).verdict, "page-rejected");
  f.input.blockedUniqueIds = []; f.host.tick(); f.echo(); f.host.tick();
  assert.equal(f.intents.length, 1, "unblocking and polling never retry a previously refused action");
  assert.equal(f.emit(painted).accepted, false); assert.equal(f.intents.length, 1);
  assert.equal(f.host.diagnostic().intentVerdicts.at(-1).verdict, "host-rejected");
});

test("malformed and retired sink evidence cannot retain raw payload or grant an item action", () => {
  const f = resourceFixture(); f.activate();
  const sink = f.sink, painted = f.intent();
  const malformed = JSON.parse(sink(JSON.stringify({ type: "private-unknown-type", modelRevision: "private-token", source: { uniqueId: 999 } })));
  assert.equal(malformed.accepted, false); assert.equal(f.intents.length, 0);
  const evidence = f.host.diagnostic().intentVerdicts.at(-1);
  assert.deepEqual(evidence, { verdict: "host-rejected", type: null, hostModelRevision: painted.modelRevision,
    nativeModelRevision: null, hostPresentationRevision: painted.presentationRevision, nativePresentationRevision: null, intentSequence: null });
  f.host.stop();
  assert.equal(JSON.parse(sink(JSON.stringify(painted))).accepted, false);
  assert.equal(f.intents.length, 0); assert.equal(f.host.diagnostic().intentVerdicts.at(-1).verdict, "host-rejected");
  assert.equal(JSON.stringify(f.host.diagnostic().intentVerdicts).includes("private-"), false);
});
