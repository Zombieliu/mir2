import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const page = ts.createSourceFile(
  "page.tsx",
  readFileSync(process.env.MIR2_ROUTE_TEST_PAGE ?? new URL("../app/page.tsx", import.meta.url), "utf8"),
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TSX,
);
const declared = new Map();
let secondary;
function collect(node) {
  if (ts.isFunctionDeclaration(node) && node.name) declared.set(node.name.text, node.getText(page));
  if (ts.isBinaryExpression(node) && node.left.getText(page) === "onViewportTileSecondaryActionRef.current") {
    secondary = node.right.getText(page);
  }
  ts.forEachChild(node, collect);
}
collect(page);
assert(secondary);

const handlers = [
  "moveToTile", "queueCrystalMoveIntent", "trySendQueuedCrystalMove", "reconcileSelfMovementAck",
  "dropReleasedViewportStep", "rememberBlockedDirectionAtSource", "cancelMapImageRoute", "cancelQuestRoute",
];
for (const name of handlers) assert(declared.has(name), name);
const functions = handlers.concat(["newMoveOwnerCurrent", "keepCorrectedRoute"].filter(name => declared.has(name)));
const compiled = ts.transpileModule(
  functions.map(name => declared.get(name)).join("\n") + "\nconst secondaryClick = " + secondary + "\nreturn {" + handlers.join(",") + ",secondaryClick};",
  { compilerOptions: { target: ts.ScriptTarget.ES2022 } },
).outputText;
const fixtureApi = new Function("scope", "with (scope) {\n" + compiled + "\n}");

const core = {};
new Function("exports", ts.transpileModule(
  readFileSync(new URL("../app/components/original-client-movement-controller.ts", import.meta.url), "utf8"),
  { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } },
).outputText)(core);

function fixture() {
  const ref = (current = null) => ({ current });
  const self = { x: 10, y: 10, direction: "Right", objectId: "7", dead: false };
  const world = { mapFileName: "0", playerObjectId: "7", connected: true, entities: [self] };
  const socket = { readyState: 1 };
  const lifetime = { socket, identity: { playerObjectId: 7, mapFileName: "0", sceneRevision: 1, sessionGeneration: 1, connectionGeneration: 1 } };
  const sends = [], plans = [], corrections = [], occupied = new Set();
  const flags = { mapCurrent: true, questCurrent: true, focus: true, unknown: false, run: false };
  let clock = 1000;
  const scope = {
    ...core,
    Date: { now: () => clock },
    WebSocket: { OPEN: 1 },
    document: { visibilityState: "visible", hasFocus: () => flags.focus },
    MOVEMENT_PENDING_ACTION_MAX_AGE_MS: 3000, MOVEMENT_QUEUED_DIRECTION_MAX_AGE_MS: 1160,
    MOVEMENT_ROUTE_MAX_BLOCKED_STEPS: 12,
    worldRef: ref(world), screenRef: ref("game"), socketRef: ref(socket),
    playerUiPreferencesRef: ref({ newMove: true }),
    queuedMoveIntentRef: ref(), worldFishingQueuedRef: ref(), questRouteRunRef: ref(),
    mapImageRouteRef: ref(), pendingPickupRef: ref(), combatApproachIntentRef: ref(),
    pendingSelfMoveRef: ref(), pendingSelfTurnRef: ref(), sentSelfMoves: new WeakSet(),
    movementPlanRef: ref(), queuedDirectionStepRef: ref(), queuedDirectionStepBacklogRef: ref([]),
    nextMoveSendAtRef: ref(0), directionStepNextAtRef: ref(0), directionStepVisualUntilRef: ref(0),
    crystalRunPrimedUntilRef: ref(0), movementInputBlockedUntilRef: ref(0), movementPredictionBlockedUntilRef: ref(0),
    lastQueuedMovementAttemptRef: ref(), questWindowOpenRef: ref(false), questWorldBlockedRef: ref(false),
    questAttackHandoffRef: ref(), questMapAuthorityRef: ref({ mapIndex: 0 }),
    runtimeRef: ref(), bevyQuestGenerationRef: ref(1), movementBlockedStepsRef: ref([]),
    lastSelfMovementAckRef: ref(), lastSelfPacketMovementAckRef: ref(), pendingTransferRef: ref(), prediction: ref(),
    currentAuthoritativeSelf: () => self,
    selfMovementActionBlocked: () => false,
    currentQuestWorldIdentity: () => lifetime.identity,
    currentQuestWorldLifetime: () => flags.questCurrent ? lifetime : null,
    sameQuestWorldLifetime: (a, b) => a === b,
    sameQuestWorldIdentity: (a, b) => a === b,
    currentQuestMapIndex: (authority) => authority?.mapIndex ?? null,
    referenceWindowsBlockGameplay: () => false,
    sceneInputDeferredForInitialAssets: () => false,
    observeQuestWorldActions: () => {},
    readBevyQuestUiStatus: () => null,
    mapImageRouteCurrent: route => flags.mapCurrent && scope.mapImageRouteRef.current === route,
    crystalEffectiveMovementMode: () => flags.run ? "run" : "walk",
    recentMovementBlockedSteps: steps => steps,
    originalMapClosedDoorIndexOnMovementPath: () => null,
    transferKeyForWorldTile: () => null,
    crystalSelfMovementTraits: () => ({}),
    clearLegacySelfMovementCoordinateSources: () => {},
    setPredictedPlayerMotion: point => { scope.prediction.current = point; },
    clearLocalSelfPrediction: () => { scope.prediction.current = null; },
    observeBevyMovementShadowCommand: () => {},
    scheduleMovementConfirmTick: () => {},
    cancelPendingPickup: () => {},
    cancelLockedMonsterAttack: () => {},
    stopOnchainMining: () => {},
    clearDirectionStepPendingQueue: () => {},
    recordMovementConsoleEvent: () => {},
    captureMovementConsoleState: () => ({}),
    bumpCorrectionCounter: type => corrections.push(type),
    send: command => {
      if (flags.unknown) throw Error("unknown movement send");
      sends.push({ command, at: clock, pending: scope.pendingSelfMoveRef.current });
      return true;
    },
    readSelfMovementControllerState: () => ({
      pending: scope.pendingSelfMoveRef.current, prediction: scope.prediction.current,
      nextMoveSendAt: scope.nextMoveSendAtRef.current, runPrimedUntil: scope.crystalRunPrimedUntilRef.current,
      inputBlockedUntil: scope.movementInputBlockedUntilRef.current,
    }),
    applySelfMovementControllerState: state => {
      scope.pendingSelfMoveRef.current = state.pending;
      scope.prediction.current = state.prediction;
      scope.nextMoveSendAtRef.current = state.nextMoveSendAt;
      scope.crystalRunPrimedUntilRef.current = state.runPrimedUntil;
      scope.movementInputBlockedUntilRef.current = state.inputBlockedUntil;
    },
    pointTileDistance: (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y)),
    pointMoveInDirection: core.movementPointInDirection,
    movementStepBlockedByVisibleMapCell: (point, direction) => {
      const next = core.movementPointInDirection(point, direction, 1);
      return occupied.has(next.x + "," + next.y);
    },
    movementStepBlockedByVisibleEntity: () => false,
  };
  function plan(origin, target, mode) {
    plans.push({ origin: origin.x, target: target.x, mode });
    if (origin.x === target.x) return null;
    const first = core.movementPointInDirection(origin, "Right", 1);
    if (occupied.has(first.x + "," + first.y)) {
      return { point: { x: origin.x, y: origin.y + 1 }, direction: "Down", mode: "walk" };
    }
    if (mode === "run" && target.x - origin.x >= 2) {
      const second = core.movementPointInDirection(origin, "Right", 2);
      if (!occupied.has(second.x + "," + second.y)) return { point: second, direction: "Right", mode };
    }
    return { point: first, direction: "Right", mode: "walk" };
  }
  scope.crystalMovementActionTowardWithRouteHints = plan;
  // The two map surfaces own the same durable queued target, with separate provenance.
  for (const name of ["questRouteMovement", "mapImageRouteMovement"]) {
    scope[name] = (route, origin, mode) => plan(origin, { x: route.queued.targetX }, mode);
  }
  const api = fixtureApi(scope);
  function start(kind) {
    if (kind === "ground") return api.secondaryClick(14, 10);
    if (kind === "classic") return api.moveToTile(14, 10, "run", "direction", "manual-step");
    const queued = { kind: "target", targetX: 14, targetY: 10, requestedMode: "run", requestedAt: clock, consumeAfterSend: false };
    if (kind === "map") scope.mapImageRouteRef.current = { queued };
    if (kind === "quest") scope.questRouteRunRef.current = { queued, lifetime, action: { mapIndex: 0 } };
    if (kind === "pickup") scope.pendingPickupRef.current = { queued, lifetime };
    api.queueCrystalMoveIntent(queued);
  }
  function tick(at) {
    clock = at;
    return api.trySendQueuedCrystalMove(clock);
  }
  function ack(at, x, packet = "UserLocation", ordinaryStep = true) {
    clock = at;
    self.x = x;
    return api.reconcileSelfMovementAck({ x, y: self.y, direction: self.direction }, packet, clock, ordinaryStep);
  }
  return { scope, sends, plans, corrections, flags, occupied, lifetime, start, tick, ack };
}

for (const kind of ["ground", "map", "quest"]) {
  test(kind + " route keeps its goal after a rejected second step and resumes from authoritative location", () => {
    const f = fixture();
    f.start(kind);
    const queue = f.scope.queuedMoveIntentRef.current;
    assert.equal(f.ack(1050, 11), "confirmed");
    assert.equal(f.tick(1600), true);
    assert.equal(f.ack(1650, 11), "correction");
    assert.strictEqual(f.scope.queuedMoveIntentRef.current, queue);
    assert.deepEqual(f.scope.movementBlockedStepsRef.current, [], "an unchanged location alone does not establish collision");
    assert.equal(f.tick(2199), false);
    assert.equal(f.sends.length, 2);
    assert.equal(f.tick(2200), true);
    assert.deepEqual(f.plans.at(-1), { origin: 11, target: 14, mode: "walk" });
    assert.equal(f.ack(2250, 12), "confirmed");
    assert.equal(f.tick(2800), true);
    assert.equal(f.ack(2850, 13), "confirmed");
    assert.equal(f.tick(3400), true);
    assert.equal(f.ack(3450, 14), "confirmed");
    assert.equal(f.tick(4000), false);
    assert.equal(f.scope.queuedMoveIntentRef.current, null);
    assert.equal(f.sends.length, 5);
  });
}

test("a live rejected Run endpoint leaves the first legal Walk open", () => {
  const f = fixture();
  f.start("ground");
  assert.equal(f.ack(1050, 11), "confirmed");
  f.flags.run = true;
  assert.equal(f.tick(1600), true);
  assert.equal(f.sends.at(-1).pending.to.x, 13);
  f.occupied.add("13,10");
  assert.equal(f.ack(1650, 11), "correction");
  assert.deepEqual(f.scope.movementBlockedStepsRef.current.map(edge => [edge.fromX, edge.fromY, edge.mode]), [
    [12, 10, "walk"], [12, 10, "run"],
  ]);
  f.flags.run = false;
  assert.equal(f.tick(2200), true);
  assert.equal(f.sends.at(-1).pending.to.x, 12);
});

test("Classic short steps, pickup approaches and unknown sends never gain retry authority", () => {
  for (const kind of ["classic", "pickup"]) {
    const f = fixture();
    f.start(kind);
    assert.equal(f.sends.length, 1);
    assert.equal(f.ack(1050, 10), "correction");
    assert.equal(f.scope.queuedMoveIntentRef.current, null);
    assert.equal(f.tick(2200), false);
  }
  for (const resolve of [true, false]) {
    const f = fixture();
    f.flags.unknown = true;
    assert.throws(() => f.start("ground"), /unknown movement send/);
    assert.equal(f.sends.length, 0);
    f.flags.unknown = false;
    if (resolve) assert.equal(f.ack(1050, 10), "correction");
    assert.equal(f.tick(4100), false);
    assert.equal(f.scope.queuedMoveIntentRef.current, null);
    assert.equal(f.sends.length, 0);
  }
});

test("wrong scene, disabled NewMove, retired map and Q sources, and unproven corrections stop", () => {
  const checks = [
    ["ground", f => { f.lifetime.identity = { ...f.lifetime.identity, sceneRevision: 2 }; }],
    ["ground", f => { f.scope.socketRef.current = { readyState: 1 }; }],
    ["ground", f => { f.scope.playerUiPreferencesRef.current.newMove = false; }],
    ["map", f => { f.flags.mapCurrent = false; }],
    ["quest", f => { f.flags.questCurrent = false; }],
    ["quest", f => { f.scope.questWorldBlockedRef.current = true; }],
  ];
  for (const [kind, retire] of checks) {
    const f = fixture();
    f.start(kind);
    retire(f);
    assert.equal(f.ack(1050, 10), "correction");
    assert.equal(f.tick(2200), false);
    assert.equal(f.scope.queuedMoveIntentRef.current, null);
    assert.equal(f.sends.length, 1);
  }
  for (const [x, packet, ordinary] of [[10, "UserLocation", false], [10, "UserDashFail", false], [9, "UserLocation", true]]) {
    const f = fixture();
    f.start("ground");
    assert.equal(f.ack(1050, x, packet, ordinary), "correction");
    assert.equal(f.tick(2200), false);
    assert.equal(f.sends.length, 1);
  }
});

test("a ground owner retired during prediction cannot enter the socket", () => {
  const f = fixture();
  const record = f.scope.setPredictedPlayerMotion;
  f.scope.setPredictedPlayerMotion = point => {
    record(point);
    f.lifetime.identity = { ...f.lifetime.identity, sceneRevision: 2 };
  };
  f.start("ground");
  assert.deepEqual(f.sends, []);
  assert.equal(f.scope.queuedMoveIntentRef.current, null);
  assert.equal(f.scope.pendingSelfMoveRef.current, null);
  assert.equal(f.scope.prediction.current, null);
});
