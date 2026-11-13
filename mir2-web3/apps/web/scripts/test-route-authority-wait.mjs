import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function load(relative) {
  const url = new URL(relative, import.meta.url), module = { exports: {} };
  const code = ts.transpileModule(readFileSync(url, "utf8"), { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText;
  new Function("require", "module", "exports", code)(name => load(new URL(name + ".ts", url)), module, module.exports);
  return module.exports;
}
const controller = load("../app/components/original-client-movement-controller.ts");
const lifecycle = load("../lib/quest-world-lifecycle.ts");
const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const tree = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const names = ["trySendQueuedCrystalMove", "dropReleasedViewportStep", "observeQuestWorldActions",
  "cancelQuestRoute", "finishQuestRoute", "cancelPendingPickup", "cancelMapImageRoute", "queueCrystalMoveIntent"];
const constants = ["MOVEMENT_PENDING_ACTION_MAX_AGE_MS", "AUTO_ROUTE_PENDING_MAX_AGE_MS"];
const declarations = [];
function visit(node) {
  if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) declarations.push(node.getText(tree));
  if (ts.isVariableDeclaration(node) && constants.includes(node.name.getText(tree))) declarations.push("const " + node.getText(tree) + ";");
  ts.forEachChild(node, visit);
}
visit(tree);
assert.equal(declarations.length, names.length + constants.length);
const code = ts.transpileModule(declarations.join("\n"), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;

function harness(kind = "quest") {
  const ref = current => ({ current });
  const identity = { connectionGeneration: 1, sessionGeneration: 1, sceneRevision: 1, playerObjectId: 1000, mapFileName: "0" };
  const lifetime = { identity, generation: 1, runtime: {}, runtimeGeneration: 1, socket: {}, questSource: "cat0/2" };
  const state = { now: 1000, self: { x: 1, y: 1, direction: "Right" }, live: lifetime, focused: true, mapValid: true, newMoveValid: true };
  const pending = controller.createPendingSelfMove({ from: state.self, direction: "Right", requestedMode: "walk", now: state.now, runPrimedUntil: 0 });
  const queued = kind === "manual" ? { kind: "direction", direction: "Right", requestedMode: "walk", requestedAt: 1000, consumeAfterSend: true }
    : { kind: "target", targetX: 8, targetY: 1, requestedMode: "run", requestedAt: 1000, consumeAfterSend: false,
      ...(kind === "newMove" ? { newMoveOwner: { identity, socket: lifetime.socket } } : {}) };
  const route = { queued, lifetime, action: { mapIndex: 0, routeTarget: { type: "huntRegion" }, x: 10, y: 10 } };
  const refs = { queued: ref(queued), pending: ref(pending), quest: ref(kind === "quest" ? route : null), map: ref(kind === "map" ? route : null) };
  const sends = [], logs = [], plans = [], counters = []; let ticks = 0, predictionsCleared = 0;
  const currentStep = (_route, self) => { plans.push({ ...self }); return { point: { x: self.x + 1, y: self.y }, direction: "Right", mode: "walk" }; };
  const deps = {
    Date: { now: () => state.now }, window: { clearTimeout() {} },
    MOVEMENT_PENDING_MAX_AGE_MS: controller.MOVEMENT_PENDING_MAX_AGE_MS,
    queuedMoveIntentRef: refs.queued, pendingSelfMoveRef: refs.pending, questRouteRunRef: refs.quest, mapImageRouteRef: refs.map,
    movementPlanRef: ref(null), worldFishingQueuedRef: ref(null), pendingPickupRef: ref(null), questAttackHandoffRef: ref(null),
    combatApproachIntentRef: ref(null), questWindowOpenRef: ref(false), bevyQuestGenerationRef: ref(1), runtimeRef: ref({}),
    questWorldBlockedRef: ref(false), questMapAuthorityRef: ref({ ...identity, mapIndex: 0 }),
    worldRef: ref({ entities: [], groundDrops: [], mapFileName: "0", mapTransfers: [] }),
    document: { visibilityState: "visible", hasFocus: () => state.focused },
    currentQuestWorldLifetime: () => state.live, sameQuestWorldLifetime: lifecycle.sameQuestWorldLifetime,
    currentQuestMapIndex: lifecycle.currentQuestMapIndex, questAttackTargetCurrent: lifecycle.questAttackTargetCurrent,
    mapImageRouteCurrent: () => state.mapValid && state.focused, newMoveOwnerCurrent: () => state.newMoveValid && state.focused,
    currentAuthoritativeSelf: () => state.self, sceneInputDeferredForInitialAssets: () => false, readBevyQuestUiStatus: () => null,
    lastQueuedMovementAttemptRef: ref(null), scheduleMovementConfirmTick: () => { ticks++; },
    bumpCorrectionCounter: (name, value) => counters.push({ name, value }), clearLegacySelfMovementCoordinateSources() {},
    clearLocalSelfPrediction: () => { predictionsCleared++; },
    crystalRunPrimedUntilRef: ref(0), movementInputBlockedUntilRef: ref(0), CRYSTAL_CORRECTION_BLOCK_MS: 400,
    readSelfMovementControllerState: () => ({ pending: refs.pending.current, nextMoveSendAt: 0, inputBlockedUntil: 0 }),
    canSendMovement: controller.canSendMovement, selfMovementActionBlocked: () => false, MOVEMENT_QUEUED_DIRECTION_MAX_AGE_MS: 3000,
    crystalEffectiveMovementMode: mode => mode, questRouteMovement: currentStep, mapImageRouteMovement: currentStep,
    crystalMovementActionTowardWithRouteHints: self => currentStep(null, self),
    crystalMovementActionForDirection: self => currentStep(null, self),
    movementBlockedStepsRef: ref([]), recentMovementBlockedSteps: () => [],
    originalMapClosedDoorIndexOnMovementPath: () => null, doorOpenRequestUntilRef: ref(new Map()), CRYSTAL_DOOR_OPEN_RETRY_MS: 4000,
    transferKeyForWorldTile: () => null, pendingTransferRef: ref(null), crystalSelfMovementTraits: () => ({}),
    crystalMovementProfile: controller.crystalMovementProfile, createPendingSelfMove: controller.createPendingSelfMove,
    pendingSelfTurnRef: ref(null), nextMoveSendAtRef: ref(0), directionStepNextAtRef: ref(0), directionStepVisualUntilRef: ref(0),
    setPredictedPlayerMotion() {}, send: command => { sends.push(command); return true; }, sentSelfMoves: new WeakSet(),
    observeBevyMovementShadowCommand() {}, queuedDirectionStepRef: ref(null), queuedDirectionStepBacklogRef: ref([]), clearDirectionStepPendingQueue() {},
    t: key => key, appendLog: text => logs.push(text),
  };
  const api = new Function(...Object.keys(deps), `${code}; return {${names.join(",")}};`)(...Object.values(deps));
  const advance = age => { state.now = pending.sentAt + age; return api.trySendQueuedCrystalMove(state.now); };
  const ack = value => {
    const result = controller.reconcileMovementAck({ state: { pending: refs.pending.current, prediction: null, nextMoveSendAt: 0, inputBlockedUntil: 0, runPrimedUntil: 0 },
      ack: value, packetName: "UserLocation", now: state.now });
    refs.pending.current = result.state.pending; state.self = { ...value }; return result.outcome;
  };
  return { ...api, kind, state, refs, queued, pending, route, sends, logs, plans, counters, advance, ack,
    ticks: () => ticks, predictionsCleared: () => predictionsCleared, deps };
}

for (const kind of ["quest", "map", "newMove"]) test(`${kind}: a seven-second in-flight result keeps one owned route, never duplicates a move, then resumes from authority`, () => {
  const h = harness(kind);
  for (const age of [0, 3000, 3001, 6938, 7999]) {
    assert.equal(h.advance(age), false); assert.equal(h.refs.pending.current, h.pending);
    assert.equal(h.refs.queued.current, h.queued); assert.equal(h.sends.length, 0); assert.equal(h.plans.length, 0);
  }
  assert(h.ticks() >= 5); assert.equal(h.predictionsCleared(), 0); assert.deepEqual(h.counters, []);
  h.state.now = h.pending.sentAt + 7000;
  assert.equal(h.ack(h.pending.to), "confirmed");
  assert.equal(h.trySendQueuedCrystalMove(h.state.now), true);
  assert.deepEqual(h.plans, [h.pending.to], "resume uses the late authoritative endpoint rather than replaying the old step");
  assert.equal(h.sends.length, 1); assert.equal(h.refs.queued.current, h.queued);
});

for (const kind of ["quest", "map", "newMove"]) test(`${kind}: no confirmation at eight seconds retires owner and queue with one localized notice`, () => {
  const h = harness(kind); assert.equal(h.advance(7999), false); assert.equal(h.advance(8000), false);
  assert.equal(h.refs.pending.current, null); assert.equal(h.refs.queued.current, null);
  assert.equal(h.refs.quest.current, null); assert.equal(h.refs.map.current, null);
  assert.deepEqual(h.logs, ["ui.routeConfirmationTimeout"]); assert.equal(h.sends.length, 0);
  assert.equal(h.advance(9000), false); assert.equal(h.logs.length, 1);
  assert.equal(h.ack(h.pending.to), "accepted"); assert.equal(h.trySendQueuedCrystalMove(11000), false);
  assert.equal(h.sends.length, 0, "an expired route cannot revive on a later ACK");
});

for (const kind of ["manual", "unownedTarget"]) test(`${kind}: the original three-second timeout remains`, () => {
  const h = harness(kind); assert.equal(h.advance(3001), false);
  assert.equal(h.refs.pending.current, null); assert.equal(h.refs.queued.current, null);
  assert.equal(h.counters[0].name, "pendingTimeout"); assert.deepEqual(h.logs, []); assert.deepEqual(h.sends, []);
});

for (const kind of ["quest", "map", "newMove"]) test(`${kind}: manual replacement during grace cancels the old route and late ACK cannot resurrect it`, () => {
  const h = harness(kind); h.advance(4000);
  const manual = { kind: "direction", direction: "Left", requestedMode: "walk", requestedAt: h.state.now, consumeAfterSend: true };
  h.queueCrystalMoveIntent(manual);
  assert.equal(h.refs.quest.current, null); assert.equal(h.refs.map.current, null);
  assert.equal(h.refs.queued.current, null); assert.equal(h.refs.pending.current, null);
  assert.equal(h.ack(h.pending.to), "accepted"); assert.equal(h.trySendQueuedCrystalMove(h.state.now + 3000), false);
  assert.deepEqual(h.sends, []);
});

for (const kind of ["quest", "map", "newMove"]) test(`${kind}: lost ownership or focus retires the queue before grace can retain it`, () => {
  for (const change of ["owner", "focus"]) {
    const h = harness(kind); h.advance(4000);
    if (change === "focus") h.state.focused = false;
    else { h.state.live = null; h.state.mapValid = false; h.state.newMoveValid = false; }
    assert.equal(h.advance(4500), false); assert.equal(h.refs.queued.current, null);
    h.state.focused = true; h.state.live = h.route.lifetime; h.state.mapValid = h.state.newMoveValid = true;
    h.ack(h.pending.to); assert.equal(h.trySendQueuedCrystalMove(8000), false); assert.deepEqual(h.sends, []);
  }
});

test("a newly chosen route may wait for a previous manual in-flight move, then plans only the new destination", () => {
  const h = harness("manual");
  const target = { kind: "target", targetX: 15, targetY: 1, requestedMode: "run", requestedAt: h.state.now, consumeAfterSend: false };
  const route = { ...h.route, queued: target, action: { ...h.route.action, x: 15, y: 1 } };
  h.refs.quest.current = route; h.queueCrystalMoveIntent(target);
  assert.equal(h.refs.pending.current, h.pending); assert.equal(h.refs.queued.current, target);
  assert.equal(h.advance(6938), false); assert.equal(h.sends.length, 0);
  assert.equal(h.ack(h.pending.to), "confirmed"); assert.equal(h.trySendQueuedCrystalMove(h.state.now), true);
  assert.deepEqual(h.plans, [h.pending.to]); assert.equal(h.sends.length, 1); assert.equal(h.refs.quest.current, route);
});

test("a delayed run degraded to one walk tile confirms and resumes from the actual shorter endpoint", () => {
  const h = harness("quest");
  h.refs.pending.current = controller.createPendingSelfMove({ from: h.state.self, direction: "Right", requestedMode: "run", now: h.state.now, runPrimedUntil: h.state.now + 1200 });
  assert.equal(h.refs.pending.current.mode, "run"); assert.equal(h.refs.pending.current.to.x, 3);
  h.advance(6938); assert.equal(h.sends.length, 0);
  const degraded = { x: 2, y: 1, direction: "Right" };
  assert.equal(h.ack(degraded), "confirmed"); assert.equal(h.trySendQueuedCrystalMove(h.state.now), true);
  assert.deepEqual(h.plans, [degraded]); assert.equal(h.sends.length, 1);
});
