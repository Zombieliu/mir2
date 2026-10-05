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
const bag = load("../lib/bevy-bag-ui.ts", () => identity);
const model = () => ({ capacity: 46, gold: 200, items: [{ container: 0, slot: 0, uniqueId: 0 }] });
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false };

function fixture(overrides = {}) {
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
    onState(next) { state = next; }, onIntent(intent) { intents.push(intent); return { accepted: true }; },
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
