import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function load(name, dependencies = {}) {
  const exports = {};
  const source = readFileSync(new URL(`../lib/${name}.ts`, import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  new Function("exports", "require", code)(exports, key => {
    assert(Object.hasOwn(dependencies, key), key); return dependencies[key];
  });
  return exports;
}
const fishing = load("world-fishing-source");
const { nativeAttackMovementChannel } = load("native-attack-movement", { "./world-fishing-source": fishing });
const capability = JSON.stringify({ type: "attackCapabilitiesV2", protocolVersion: 2,
  capability: "nativeAttackOutcomeV1", supported: true, scope: "connection" });
function fixture() {
  const owner = { socket: {}, connectionGeneration: 1, sessionGeneration: 2, sceneRevision: 3, playerObjectId: 7, mapFileName: "0" };
  const store = new fishing.WorldFishingSource();
  store.observeSnapshot(owner, { playerObjectId: 7, mapFileName: "0", entities: [
    { objectId: 7, kind: "selfPlayer", x: 10, y: 10, direction: "Right", dead: false, fishing: false, transformType: 0 },
  ] });
  const channel = nativeAttackMovementChannel(owner.socket);
  channel.declare(); channel.observeFrame(capability, owner);
  const proof = () => ({ identity: owner, map: "0", targetId: 14, movementBlockMs: 1386, command: { type: "attack", objectId: 14 } });
  const send = (at, p = proof()) => channel.enter(p, store.current(owner), 14, at);
  const frame = (result, code, requestId = "1", objectId = 14, source = owner) => channel.observeFrame(JSON.stringify({
    type: "attackResultV2", protocolVersion: 2, requestId, objectId, result, ...(code === undefined ? {} : { code }),
  }), source);
  const body = at => {
    const previous = store.current(owner);
    store.observePacket(owner, "ObjectAttack", { objectId: 7, location: { x: 10, y: 10 }, direction: "Right" }, at);
    channel.observeBody(owner, previous, store.current(owner), at);
  };
  return { owner, channel, proof, send, frame, body, current: () => channel.current(owner), receipt: () => channel.receipt(channel.current(owner)) };
}

for (const code of ["unsupported", "executionUnavailable"]) test(`${code} gives one no-entry hint without replaying on this socket`, () => {
  const f = fixture(), p = f.proof(); f.send(1000, p); f.channel.finish(p, 1020, false);
  assert.equal(f.frame("unavailable", code, "2"), null);
  assert.equal(f.frame("unavailable", code, "1", 15), null);
  assert.equal(f.frame("unavailable", code, "1", 14, { ...f.owner, socket: {} }), null);
  assert.equal(f.frame("unavailable", code, "1", 14, { ...f.owner, sceneRevision: 9 }), null);
  const hint = f.frame("unavailable", code);
  assert(Object.isFrozen(hint)); assert.equal(hint.owner, f.current().source.owner); assert.equal(hint.previousProof, p);
  assert.deepEqual([hint.requestId, hint.objectId, hint.notBeforeMs, hint.code], ["1", 14, 2406, code]);
  assert.equal(f.frame("unavailable", code), null);
  assert.equal(f.send(5000), null); assert.equal(f.channel.enterLegacy(), false);
});

for (const [result, code] of [["unavailable", "authenticationRequired"], ["unavailable", "requestIdNotFresh"],
  ["unknown", "outcomeUnknown"], ["unknown", "previousOutcomeUnknown"]]) test(`${result}/${code} cannot allow replay or downgrade`, () => {
  const f = fixture(); f.send(1000); assert.equal(f.frame(result, code), null);
  assert.equal(f.send(3500), null); assert.equal(f.channel.enterLegacy(), false);
  assert.equal(f.frame("unavailable", "unsupported"), null);
});

test("duplicate decoded keys, invalid codes and missing outcomes do not settle an attack", () => {
  const f = fixture(); f.send(1000);
  const raw = '{"type":"attackResultV2","protocolVersion":2,"requestId":"1","request\\u0049d":"1","objectId":14,"result":"unavailable","code":"unsupported"}';
  assert.equal(f.channel.observeFrame(raw, f.owner), null);
  assert.equal(f.frame("unavailable", "bogus"), null); assert.equal(f.current().terminal, null);
  assert.equal(f.send(4000), null); assert.equal(f.channel.enterLegacy(), false);
});

test("an observed action body prevents an unavailable response from permitting legacy recovery", () => {
  const f = fixture(); f.send(1000); f.body(1010);
  assert.equal(f.frame("unavailable", "executionUnavailable"), null);
  assert.equal(f.receipt(), null); assert.equal(f.channel.enterLegacy(), false);
});

test("notLaunched permits a new proof only after the full attack interval", () => {
  const f = fixture(), p = f.proof(); f.send(1000, p); f.frame("notLaunched");
  assert.equal(f.send(2385), null); assert.equal(f.send(2386, p), null);
  assert.equal(JSON.parse(f.send(2386)).requestId, "2"); assert.equal(f.channel.enterLegacy(), false);
});

test("a lost body does not freeze future attacks or qualify its late body for a newer request", () => {
  const f = fixture(), p = f.proof(); f.send(1000, p); f.frame("launched");
  assert.equal(f.receipt(), null); assert.equal(f.send(2385), null); assert.equal(f.send(2386, p), null);
  assert.equal(JSON.parse(f.send(2386)).requestId, "2");
  f.body(2390); f.body(2400); f.frame("launched", undefined, "2");
  assert.equal(f.receipt(), null); assert.equal(f.current().until, 3772);
  assert.equal(f.send(3771), null); assert.equal(JSON.parse(f.send(3772)).requestId, "3");
  assert.equal(f.channel.enterLegacy(), false);
});

test("a body received before the next attack permits independent later qualification", () => {
  const f = fixture(); f.send(1000); f.frame("launched"); f.body(2400); assert(f.receipt());
  assert.equal(JSON.parse(f.send(2410)).requestId, "2"); f.frame("launched", undefined, "2");
  assert.equal(f.receipt(), null); f.body(2420); assert.equal(f.receipt().actionEvidence.receivedAtMs, 2420);
});

test("a transport throw remains ambiguous and cannot silently retry", () => {
  const f = fixture(), p = f.proof(); f.send(1000, p); assert.equal(f.channel.finish(p, 1015, true), true);
  assert.equal(f.send(5000), null); assert.equal(f.channel.enterLegacy(), false);
  f.frame("unknown", "outcomeUnknown"); assert.equal(f.send(5001), null); assert.equal(f.receipt(), null);
});

test("old servers keep the ordinary lane and cannot switch an already legacy socket to typed", () => {
  const socket = {}, channel = nativeAttackMovementChannel(socket);
  channel.observeFrame(capability, null); assert.equal(channel.usesTypedWire, false);
  assert.equal(channel.enterLegacy(), true); channel.declare(); channel.observeFrame(capability, null);
  assert.equal(channel.usesTypedWire, false);
});
